// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use common_datavalues::DataTypePtr;
use common_exception::ErrorCode;
use common_exception::Result;

pub fn assert_string(data_type: &DataTypePtr) -> Result<()> {
    if !data_type.data_type_id().is_string() {
        return Err(ErrorCode::IllegalDataType(format!(
            "Expected a string type, but got {:?}",
            data_type
        )));
    }
    Ok(())
}

pub fn assert_numeric(data_type: &DataTypePtr) -> Result<()> {
    if !data_type.data_type_id().is_numeric() {
        return Err(ErrorCode::IllegalDataType(format!(
            "Expected a numeric type, but got {:?}",
            data_type
        )));
    }
    Ok(())
}

pub fn assert_date_or_datetime(data_type: &DataTypePtr) -> Result<()> {
    if !data_type.data_type_id().is_date_or_date_time() {
        return Err(ErrorCode::IllegalDataType(format!(
            "Expected a data or datetime type, but got {:?}",
            data_type
        )));
    }
    Ok(())
}
