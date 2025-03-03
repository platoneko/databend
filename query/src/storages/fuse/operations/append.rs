//  Copyright 2021 Datafuse Labs.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//

use std::str::FromStr;
use std::sync::Arc;

use async_stream::stream;
use common_cache::Cache;
use common_exception::ErrorCode;
use common_exception::Result;
use common_streams::SendableDataBlockStream;
use futures::StreamExt;

use crate::sessions::QueryContext;
use crate::storages::fuse::io::BlockStreamWriter;
use crate::storages::fuse::operations::AppendOperationLogEntry;
use crate::storages::fuse::FuseTable;
use crate::storages::fuse::DEFAULT_BLOCK_PER_SEGMENT;
use crate::storages::fuse::DEFAULT_ROW_PER_BLOCK;
use crate::storages::fuse::FUSE_OPT_KEY_BLOCK_PER_SEGMENT;
use crate::storages::fuse::FUSE_OPT_KEY_ROW_PER_BLOCK;

pub type AppendOperationLogEntryStream =
    std::pin::Pin<Box<dyn futures::stream::Stream<Item = Result<AppendOperationLogEntry>> + Send>>;

impl FuseTable {
    #[inline]
    pub async fn append_trunks(
        &self,
        ctx: Arc<QueryContext>,
        stream: SendableDataBlockStream,
    ) -> Result<AppendOperationLogEntryStream> {
        let rows_per_block = self.get_option(FUSE_OPT_KEY_ROW_PER_BLOCK, DEFAULT_ROW_PER_BLOCK);

        let block_per_seg =
            self.get_option(FUSE_OPT_KEY_BLOCK_PER_SEGMENT, DEFAULT_BLOCK_PER_SEGMENT);

        let da = ctx.get_storage_operator()?;

        let mut segment_stream = BlockStreamWriter::write_block_stream(
            da.clone(),
            stream,
            self.table_info.schema().clone(),
            rows_per_block,
            block_per_seg,
            self.meta_location_generator().clone(),
        )
        .await;

        let locs = self.meta_location_generator().clone();
        let segment_info_cache = ctx.get_storage_cache_manager().get_table_segment_cache();

        let log_entries = stream! {
            while let Some(segment) = segment_stream.next().await {
                let log_entry_res = match segment {
                    Ok(seg) => {
                        let seg_loc = locs.gen_segment_info_location();
                        let bytes = serde_json::to_vec(&seg)?;
                        da.object(&seg_loc)
                        .writer()
                        .write_bytes(bytes)
                        .await
                        .map_err(|e| ErrorCode::DalTransportError(e.to_string()))?;
                        let seg = Arc::new(seg);
                        let log_entry = AppendOperationLogEntry::new(seg_loc.clone(), seg.clone());
                        if let Some(ref cache) = segment_info_cache {
                            let cache = &mut cache.write().await;
                            cache.put(seg_loc, seg);
                        }

                        Ok(log_entry)
                    },
                    Err(err) => Err(err),
                };
                yield(log_entry_res);
            }
        };

        Ok(Box::pin(log_entries))
    }

    fn get_option<T: FromStr>(&self, opt_key: &str, default: T) -> T {
        self.table_info
            .options()
            .get(opt_key)
            .and_then(|s| s.parse::<T>().ok())
            .unwrap_or(default)
    }
}
