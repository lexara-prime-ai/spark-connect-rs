//! Spark Catalog representation through which the user may create, drop, alter or query underlying databases, tables, functions, etc.

use std::collections::HashMap;

use arrow::array::RecordBatch;

use crate::errors::SparkError;
use crate::plan::LogicalPlanBuilder;
use crate::session::SparkSession;
use crate::spark::DataType;
use crate::storage::StorageLevel;
use crate::{spark, DataFrame};

#[derive(Debug, Clone)]
pub struct Catalog {
    spark_session: SparkSession,
}

impl Catalog {
    pub fn new(spark_session: SparkSession) -> Self {
        Self { spark_session }
    }

    fn arrow_to_bool(record: RecordBatch) -> Result<bool, SparkError> {
        let col = record.column(0);

        let data: &arrow::array::BooleanArray = match col.data_type() {
            arrow::datatypes::DataType::Boolean => col.as_any().downcast_ref().unwrap(),
            _ => unimplemented!("only Boolean data types are currently handled currently."),
        };

        Ok(data.value(0))
    }

    /// Returns the current default catalog in this session
    pub async fn current_catalog(self) -> Result<String, SparkError> {
        let cat_type = Some(spark::catalog::CatType::CurrentCatalog(
            spark::CurrentCatalog {},
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_first_value(plan).await
    }

    pub async fn set_current_catalog(self, catalog_name: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::SetCurrentCatalog(
            spark::SetCurrentCatalog {
                catalog_name: catalog_name.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    /// Returns a list of catalogs in this session
    pub async fn list_catalogs(self, pattern: Option<&str>) -> Result<RecordBatch, SparkError> {
        let pattern = pattern.map(|val| val.to_owned());

        let cat_type = Some(spark::catalog::CatType::ListCatalogs(spark::ListCatalogs {
            pattern,
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    /// Returns the current default database in this session
    pub async fn current_database(self) -> Result<String, SparkError> {
        let cat_type = Some(spark::catalog::CatType::CurrentDatabase(
            spark::CurrentDatabase {},
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_first_value(plan).await
    }

    pub async fn set_current_database(self, db_name: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::SetCurrentDatabase(
            spark::SetCurrentDatabase {
                db_name: db_name.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    /// Returns a list of databases in this session
    pub async fn list_databases(self, pattern: Option<&str>) -> Result<RecordBatch, SparkError> {
        let pattern = pattern.map(|val| val.to_owned());

        let cat_type = Some(spark::catalog::CatType::ListDatabases(
            spark::ListDatabases { pattern },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn get_database(self, db_name: &str) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::GetDatabase(spark::GetDatabase {
            db_name: db_name.to_string(),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn database_exists(self, db_name: &str) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::DatabaseExists(
            spark::DatabaseExists {
                db_name: db_name.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn create_table(
        self,
        table_name: &str,
        source: Option<&str>,
        schema: Option<DataType>,
        description: Option<&str>,
        options: Option<HashMap<String, String>>,
    ) -> Result<DataFrame, SparkError> {
        let source = if let Some(s) = source {
            s.to_string()
        } else {
            // If no source is provided, use the default data source from the Spark config
            let mut config = self.spark_session.conf();
            let default_source = config.get("spark.sql.sources.default", None).await?;
            default_source
        };

        let description = if let Some(d) = description {
            if d.is_empty() {
                None
            } else {
                Some(d.to_string())
            }
        } else {
            None
        };

        let create_table_message = spark::CreateTable {
            table_name: table_name.to_string(),
            source: Some(source),
            schema,
            description,
            options: options.unwrap_or_default(),
            path: None,
        };

        let cat_type = Some(spark::catalog::CatType::CreateTable(create_table_message));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.clone().client().to_arrow(plan).await?;

        let df = self.spark_session.read().table(table_name, None)?;

        Ok(df)
    }

    pub async fn create_external_table(
        self,
        table_name: &str,
        path: &str,
        source: Option<&str>,
        schema: Option<DataType>,
        options: Option<HashMap<String, String>>,
    ) -> Result<DataFrame, SparkError> {
        let source = if let Some(s) = source {
            s.to_string()
        } else {
            // If no source is provided, use the default data source from the Spark config
            let mut config = self.spark_session.conf();
            let default_source = config.get("spark.sql.sources.default", None).await?;
            default_source
        };

        let create_external_table_message = spark::CreateExternalTable {
            table_name: table_name.to_string(),
            path: Some(path.to_string()),
            source: Some(source),
            schema: schema.into(),
            options: options.unwrap_or_default(),
        };

        let cat_type = Some(spark::catalog::CatType::CreateExternalTable(
            create_external_table_message,
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.clone().client().to_arrow(plan).await?;

        let df = self.spark_session.read().table(table_name, None)?;

        Ok(df)
    }

    /// Returns a list of tables/views in the specific database
    pub async fn list_tables(
        self,
        pattern: Option<&str>,
        db_name: Option<&str>,
    ) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::ListTables(spark::ListTables {
            db_name: db_name.map(|db| db.to_owned()),
            pattern: pattern.map(|val| val.to_owned()),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn get_table(self, table_name: &str) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::GetTable(spark::GetTable {
            table_name: table_name.to_string(),
            db_name: None,
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn list_functions(
        self,
        db_name: Option<&str>,
        pattern: Option<&str>,
    ) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::ListFunctions(
            spark::ListFunctions {
                db_name: db_name.map(|val| val.to_owned()),
                pattern: pattern.map(|val| val.to_owned()),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn function_exists(
        self,
        function_name: &str,
        db_name: Option<&str>,
    ) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::FunctionExists(
            spark::FunctionExists {
                function_name: function_name.to_string(),
                db_name: db_name.map(|val| val.to_owned()),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn get_function(self, function_name: &str) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::GetFunction(spark::GetFunction {
            function_name: function_name.to_string(),
            db_name: None,
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    /// Returns a list of columns for the given tables/views in the specific database
    pub async fn list_columns(
        self,
        table_name: &str,
        db_name: Option<&str>,
    ) -> Result<RecordBatch, SparkError> {
        let cat_type = Some(spark::catalog::CatType::ListColumns(spark::ListColumns {
            table_name: table_name.to_owned(),
            db_name: db_name.map(|val| val.to_owned()),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().to_arrow(plan).await
    }

    pub async fn table_exists(
        self,
        table_name: &str,
        db_name: Option<&str>,
    ) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::TableExists(spark::TableExists {
            table_name: table_name.to_string(),
            db_name: db_name.map(|val| val.to_owned()),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn drop_temp_view(self, view_name: &str) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::DropTempView(spark::DropTempView {
            view_name: view_name.to_string(),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn drop_global_temp_view(self, view_name: &str) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::DropGlobalTempView(
            spark::DropGlobalTempView {
                view_name: view_name.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn is_cached(self, table_name: &str) -> Result<bool, SparkError> {
        let cat_type = Some(spark::catalog::CatType::IsCached(spark::IsCached {
            table_name: table_name.to_string(),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        let record = self.spark_session.client().to_arrow(plan).await?;

        Catalog::arrow_to_bool(record)
    }

    pub async fn cache_table(
        self,
        table_name: &str,
        storage_level: Option<StorageLevel>,
    ) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::CacheTable(spark::CacheTable {
            table_name: table_name.to_string(),
            storage_level: storage_level.map(|val| val.to_owned().into()),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    pub async fn uncache_table(self, table_name: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::UncacheTable(spark::UncacheTable {
            table_name: table_name.to_string(),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    pub async fn clear_cache(self) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::ClearCache(spark::ClearCache {}));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    pub async fn refresh_table(self, table_name: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::RefreshTable(spark::RefreshTable {
            table_name: table_name.to_string(),
        }));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    pub async fn recover_partitions(self, table_name: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::RecoverPartitions(
            spark::RecoverPartitions {
                table_name: table_name.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }

    pub async fn refresh_by_path(self, path: &str) -> Result<(), SparkError> {
        let cat_type = Some(spark::catalog::CatType::RefreshByPath(
            spark::RefreshByPath {
                path: path.to_string(),
            },
        ));

        let rel_type = spark::relation::RelType::Catalog(spark::Catalog { cat_type });

        let plan = LogicalPlanBuilder::from(rel_type).plan_root();

        self.spark_session.client().execute_command(plan).await
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::readwriter::ParquetOptions;
    use crate::types::{StructField, StructType};
    use crate::SparkSessionBuilder;
    use crate::{errors::SparkError, types::DataType};

    async fn setup() -> SparkSession {
        println!("SparkSession Setup");

        let connection = "sc://127.0.0.1:15002/;user_id=rust_catalog";

        SparkSessionBuilder::remote(connection)
            .build()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_current_catalog() -> Result<(), SparkError> {
        let spark = setup().await;

        let value = spark.catalog().current_catalog().await?;

        assert_eq!(value, "spark_catalog".to_string());
        Ok(())
    }

    #[tokio::test]
    async fn test_set_current_catalog() -> Result<(), SparkError> {
        let spark = setup().await;

        spark.catalog().set_current_catalog("spark_catalog").await?;

        assert!(true);
        Ok(())
    }

    #[tokio::test]
    #[should_panic]
    async fn test_set_current_catalog_panic() -> () {
        let spark = setup().await;

        spark
            .catalog()
            .set_current_catalog("not_a_real_catalog")
            .await
            .unwrap();

        ()
    }

    #[tokio::test]
    async fn test_list_catalogs() -> Result<(), SparkError> {
        let spark = setup().await;

        let value = spark.catalog().list_catalogs(None).await?;

        assert_eq!(2, value.num_columns());
        assert_eq!(1, value.num_rows());

        Ok(())
    }

    #[tokio::test]
    async fn test_current_database() -> Result<(), SparkError> {
        let spark = setup().await;

        let value = spark.catalog().current_database().await?;

        assert_eq!(value, "default".to_string());
        Ok(())
    }

    #[tokio::test]
    async fn test_set_current_database() -> Result<(), SparkError> {
        let spark = setup().await;

        spark.sql("CREATE SCHEMA current_db").await?;

        spark.catalog().set_current_database("current_db").await?;

        assert!(true);

        spark.sql("DROP SCHEMA current_db").await?;

        Ok(())
    }

    #[tokio::test]
    #[should_panic]
    async fn test_set_current_database_panic() -> () {
        let spark = setup().await;

        spark
            .catalog()
            .set_current_catalog("not_a_real_db")
            .await
            .unwrap();

        ()
    }

    #[tokio::test]
    async fn test_get_database() -> Result<(), SparkError> {
        let spark = setup().await;

        spark.sql("CREATE SCHEMA get_db").await?;

        let res = spark.clone().catalog().get_database("get_db").await?;

        assert_eq!(res.num_rows(), 1);

        spark.sql("DROP SCHEMA get_db").await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_database_exists() -> Result<(), SparkError> {
        let spark = setup().await;

        let res = spark.catalog().database_exists("default").await?;

        assert!(res);

        let res = spark.catalog().database_exists("not_real").await?;

        assert!(!res);
        Ok(())
    }

    #[tokio::test]
    async fn test_function_exists() -> Result<(), SparkError> {
        let spark = setup().await;

        let res = spark.catalog().function_exists("len", None).await?;

        assert!(res);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_columns() -> Result<(), SparkError> {
        let spark = setup().await;

        spark.sql("DROP TABLE IF EXISTS tmp_table").await?;

        spark
            .sql("CREATE TABLE tmp_table (name STRING, age INT) using parquet")
            .await?;

        let res = spark.catalog().list_columns("tmp_table", None).await?;

        assert_eq!(res.num_rows(), 2);

        spark.sql("DROP TABLE IF EXISTS tmp_table").await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_drop_view() -> Result<(), SparkError> {
        let spark = setup().await;

        spark
            .range(None, 2, 1, Some(1))
            .create_or_replace_global_temp_view("tmp_view")
            .await?;

        let res = spark.catalog().drop_global_temp_view("tmp_view").await?;

        assert!(res);

        spark
            .clone()
            .range(None, 2, 1, Some(1))
            .create_or_replace_temp_view("tmp_view")
            .await?;

        let res = spark.catalog().drop_temp_view("tmp_view").await?;

        assert!(res);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_table() -> Result<(), SparkError> {
        let spark: SparkSession = setup().await;

        let table_name = "test_create_table";
        let source = Some("parquet");
        let description = Some("Test table description");

        let schema = StructType::new(vec![
            StructField {
                name: "name",
                data_type: DataType::String,
                nullable: true,
                metadata: None,
            },
            StructField {
                name: "favorite_color",
                data_type: DataType::String,
                nullable: true,
                metadata: None,
            },
            StructField {
                name: "favorite_numbers",
                data_type: DataType::Array {
                    element_type: Box::new(DataType::Integer),
                    contains_null: true,
                },
                nullable: true,
                metadata: None,
            },
        ]);

        let mut options = HashMap::new();
        options.insert("compression".to_string(), "snappy".to_string());

        let result = spark
            .catalog()
            .create_table(
                table_name,
                source,
                Some(schema.clone().into()),
                description,
                Some(options),
            )
            .await;

        let df = result.unwrap();
        let df_schema = df.clone().schema().await?;

        // Insert data
        let path = ["/opt/spark/work-dir/datasets/users.parquet"];

        let opts = ParquetOptions::default();
        let data_df = spark.read().parquet(path, opts)?;

        data_df.write().insert_tnto(table_name).await?;

        let res = spark
            .catalog()
            .list_tables(Some("test_create_table"), None)
            .await?;

        assert_eq!(res.num_rows(), 1);
        assert_eq!(df_schema, schema.into());
        assert_eq!(source.unwrap(), "parquet");

        Ok(())
    }

    #[tokio::test]
    async fn test_create_external_table() -> Result<(), SparkError> {
        let spark: SparkSession = setup().await;

        let table_name = "test_create_external_table";
        let source = Some("parquet");

        let schema = StructType::new(vec![
            StructField {
                name: "name",
                data_type: DataType::String,
                nullable: true,
                metadata: None,
            },
            StructField {
                name: "favorite_color",
                data_type: DataType::String,
                nullable: true,
                metadata: None,
            },
            StructField {
                name: "favorite_numbers",
                data_type: DataType::Array {
                    element_type: Box::new(DataType::Integer),
                    contains_null: true,
                },
                nullable: true,
                metadata: None,
            },
        ]);

        let mut options = HashMap::new();
        options.insert("compression".to_string(), "snappy".to_string());

        let path = "/opt/spark/work-dir/datasets/users.parquet";

        let result = spark
            .catalog()
            .create_external_table(
                table_name,
                path,
                source,
                Some(schema.clone().into()),
                Some(options),
            )
            .await;

        let df = result.unwrap();
        let df_schema = df.clone().schema().await?;

        let res = spark
            .catalog()
            .list_tables(Some("test_create_external_table"), None)
            .await?;

        assert_eq!(res.num_rows(), 1);
        assert_eq!(df_schema, schema.into());
        assert_eq!(source.unwrap(), "parquet");

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_table() -> Result<(), SparkError> {
        let spark = setup().await;

        spark
            .sql("CREATE TABLE cache_table (name STRING, age INT) using parquet")
            .await?;

        spark.catalog().cache_table("cache_table", None).await?;

        let res = spark.catalog().is_cached("cache_table").await?;

        assert!(res);

        spark.catalog().uncache_table("cache_table").await?;

        let res = spark.catalog().is_cached("cache_table").await?;

        assert!(!res);

        spark.sql("DROP TABLE cache_table").await?;
        Ok(())
    }
}
