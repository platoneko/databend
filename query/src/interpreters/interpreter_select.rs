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

use std::sync::Arc;

use common_datavalues::DataSchemaRef;
use common_exception::Result;
use common_planners::PlanNode;
use common_planners::SelectPlan;
use common_streams::SendableDataBlockStream;
use common_tracing::tracing;

use crate::interpreters::plan_schedulers;
use crate::interpreters::stream::ProcessorExecutorStream;
use crate::interpreters::Interpreter;
use crate::interpreters::InterpreterPtr;
use crate::optimizers::Optimizers;
use crate::pipelines::new::executor::PipelinePullingExecutor;
use crate::pipelines::new::NewPipeline;
use crate::pipelines::new::QueryPipelineBuilder;
use crate::sessions::QueryContext;

pub struct SelectInterpreter {
    ctx: Arc<QueryContext>,
    select: SelectPlan,
}

impl SelectInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, select: SelectPlan) -> Result<InterpreterPtr> {
        Ok(Arc::new(SelectInterpreter { ctx, select }))
    }

    fn rewrite_plan(&self) -> Result<PlanNode> {
        plan_schedulers::apply_plan_rewrite(
            Optimizers::create(self.ctx.clone()),
            &self.select.input,
        )
    }
}

#[async_trait::async_trait]
impl Interpreter for SelectInterpreter {
    fn name(&self) -> &str {
        "SelectInterpreter"
    }

    fn schema(&self) -> DataSchemaRef {
        self.select.schema()
    }

    #[tracing::instrument(level = "debug", name = "select_interpreter_execute", skip(self, _input_stream), fields(ctx.id = self.ctx.get_id().as_str()))]
    async fn execute(
        &self,
        _input_stream: Option<SendableDataBlockStream>,
    ) -> Result<SendableDataBlockStream> {
        // TODO: maybe panic?
        let settings = self.ctx.get_settings();

        if settings.get_enable_new_processor_framework()? != 0 {
            if self.ctx.get_cluster().is_empty() {
                let async_runtime = self.ctx.get_storage_runtime();
                let new_pipeline = self.execute2()?;
                let executor = PipelinePullingExecutor::try_create(async_runtime, new_pipeline)?;
                let executor_stream = Box::pin(ProcessorExecutorStream::create(executor)?);
                return Ok(Box::pin(self.ctx.try_create_abortable(executor_stream)?));
            }

            let optimized_plan = self.rewrite_plan()?;
            plan_schedulers::schedule_query(&self.ctx, &optimized_plan).await
        } else {
            let optimized_plan = self.rewrite_plan()?;
            plan_schedulers::schedule_query(&self.ctx, &optimized_plan).await
        }
    }

    fn execute2(&self) -> Result<NewPipeline> {
        let settings = self.ctx.get_settings();
        let builder = QueryPipelineBuilder::create(self.ctx.clone());

        let optimized_plan = self.rewrite_plan()?;
        let select_plan = SelectPlan {
            input: Arc::new(optimized_plan),
        };
        let mut new_pipeline = builder.finalize(&select_plan)?;
        new_pipeline.set_max_threads(settings.get_max_threads()? as usize);
        Ok(new_pipeline)
    }
}
