// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};

// This is a simplified mock implementation for the purposes of the exercise
// In a real app, you'd need to implement this properly with the actual SeaORM API

#[async_trait]
pub trait DatabaseConnectionExt {
    async fn query_one(&self, stmt: Statement) -> Result<QueryRow, DbErr>;
    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryRow>, DbErr>;
}

#[async_trait]
impl DatabaseConnectionExt for DatabaseConnection {
    async fn query_one(&self, stmt: Statement) -> Result<QueryRow, DbErr> {
        // This is a mock implementation
        // In a real world scenario, you would use the actual SeaORM API to execute the query

        // We're ignoring the result since this is a mock
        let _ = self.execute(stmt.clone()).await?;

        // Create a mock row - this is just for demonstration
        Ok(QueryRow { is_mock: true })
    }

    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryRow>, DbErr> {
        // This is a mock implementation
        // We're ignoring the result since this is a mock
        let _ = self.execute(stmt.clone()).await?;

        // Return a vector with a single mock row - in reality you'd extract multiple rows
        Ok(vec![QueryRow { is_mock: true }])
    }
}

pub struct QueryRow {
    is_mock: bool,
}

impl QueryRow {
    pub fn try_get<T, S>(&self, _column: S) -> Result<T, DbErr>
    where
        T: Default + 'static,
        S: AsRef<str>,
    {
        // This is a simplified mock implementation that always returns default values
        // In a real implementation, you would extract values from the actual result
        if self.is_mock {
            if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
                // Return mock string for any string column
                let mock_string = "mock_value".to_string();
                let boxed = Box::new(mock_string);
                unsafe {
                    return Ok(*Box::from_raw(Box::into_raw(boxed) as *mut T));
                }
            } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i64>() {
                // Return 100 for any i64 column
                let mock_i64 = 100_i64;
                let boxed = Box::new(mock_i64);
                unsafe {
                    return Ok(*Box::from_raw(Box::into_raw(boxed) as *mut T));
                }
            } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i32>() {
                // Return 50 for any i32 column
                let mock_i32 = 50_i32;
                let boxed = Box::new(mock_i32);
                unsafe {
                    return Ok(*Box::from_raw(Box::into_raw(boxed) as *mut T));
                }
            } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>() {
                // Return 0.5 for any f64 column
                let mock_f64 = 0.5_f64;
                let boxed = Box::new(mock_f64);
                unsafe {
                    return Ok(*Box::from_raw(Box::into_raw(boxed) as *mut T));
                }
            } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
                // Return true for any bool column
                let mock_bool = true;
                let boxed = Box::new(mock_bool);
                unsafe {
                    return Ok(*Box::from_raw(Box::into_raw(boxed) as *mut T));
                }
            } else {
                // For other types just return default
                Ok(T::default())
            }
        } else {
            Err(DbErr::Custom(
                "Not implemented: real database query functionality".to_string(),
            ))
        }
    }
}
