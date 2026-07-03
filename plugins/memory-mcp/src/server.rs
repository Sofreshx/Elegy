use std::sync::Arc;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router, Json, RoleServer, ServerHandler,
};

use crate::memory_tools::{
    map_store_error, parse_tool_arguments, MemoryCorrectArgs, MemoryCorrectResponse,
    MemoryDeleteArgs, MemoryDeleteResponse, MemoryListArgs, MemoryListResponse, MemoryRecallArgs,
    MemoryRecallResponse, MemoryRepository, MemorySearchArgs, MemorySearchResponse,
    MemoryStatsArgs, MemoryStatsResponse, MemoryStoreArgs, MemoryStoreResponse, MemoryUpdateArgs,
    MemoryUpdateResponse,
};

pub trait WriteAuditor: Send + Sync {
    fn audit_write(
        &self,
        request_context: &RequestContext<RoleServer>,
        tool: &'static str,
        id: &str,
        memory_repository: &MemoryRepository,
    );
}

#[derive(Clone, Default)]
pub struct NoopWriteAuditor;

impl WriteAuditor for NoopWriteAuditor {
    fn audit_write(
        &self,
        _request_context: &RequestContext<RoleServer>,
        _tool: &'static str,
        _id: &str,
        _memory_repository: &MemoryRepository,
    ) {
    }
}

#[derive(Clone)]
pub struct ElegyMemoryMcpServer {
    memory_repository: Arc<MemoryRepository>,
    write_auditor: Arc<dyn WriteAuditor>,
    tool_router: ToolRouter<Self>,
}

impl ElegyMemoryMcpServer {
    pub fn new(
        memory_repository: Arc<MemoryRepository>,
        write_auditor: Arc<dyn WriteAuditor>,
    ) -> Self {
        Self {
            memory_repository,
            write_auditor,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ElegyMemoryMcpServer {
    #[tool(
        name = "memory_search",
        description = "Search memories inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemorySearchArgs>()
    )]
    async fn memory_search(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemorySearchResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemorySearchArgs>(raw_arguments)?;
        let matches = self
            .memory_repository
            .search(&args)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemorySearchResponse::new(
            self.memory_repository.as_ref(),
            &args,
            matches,
        )))
    }

    #[tool(
        name = "memory_recall",
        description = "Recall a single memory by id inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryRecallArgs>()
    )]
    async fn memory_recall(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryRecallResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryRecallArgs>(raw_arguments)?;
        let memory = self
            .memory_repository
            .recall(&args.id)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryRecallResponse::from_memory(
            self.memory_repository.as_ref(),
            memory,
        )))
    }

    #[tool(
        name = "memory_list",
        description = "List memories inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryListArgs>()
    )]
    async fn memory_list(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryListResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryListArgs>(raw_arguments)?;
        let memories = self
            .memory_repository
            .list(&args)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryListResponse::new(
            self.memory_repository.as_ref(),
            &args,
            memories,
        )))
    }

    #[tool(
        name = "memory_stats",
        description = "Report memory stats for the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryStatsArgs>()
    )]
    async fn memory_stats(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryStatsResponse>, rmcp::ErrorData> {
        let _: MemoryStatsArgs = parse_tool_arguments(raw_arguments)?;
        let stats = self
            .memory_repository
            .stats()
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryStatsResponse::from_repository(
            self.memory_repository.as_ref(),
            stats,
        )))
    }

    #[tool(
        name = "memory_store",
        description = "Store a memory inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryStoreArgs>()
    )]
    async fn memory_store(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryStoreResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryStoreArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .store_memory(&args)
            .await
            .map_err(map_store_error)?;
        self.write_auditor.audit_write(
            &request_context,
            "memory_store",
            &response.memory.id,
            self.memory_repository.as_ref(),
        );
        Ok(Json(response))
    }

    #[tool(
        name = "memory_update",
        description = "Update an existing memory inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryUpdateArgs>()
    )]
    async fn memory_update(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryUpdateResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryUpdateArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .update_memory(&args)
            .await
            .map_err(map_store_error)?;
        self.write_auditor.audit_write(
            &request_context,
            "memory_update",
            &response.memory.id,
            self.memory_repository.as_ref(),
        );
        Ok(Json(response))
    }

    #[tool(
        name = "memory_correct",
        description = "Correct a memory through the configured gate-aware correction path",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryCorrectArgs>()
    )]
    async fn memory_correct(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryCorrectResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryCorrectArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .correct_memory(&args)
            .await
            .map_err(map_store_error)?;
        self.write_auditor.audit_write(
            &request_context,
            "memory_correct",
            &response.memory.id,
            self.memory_repository.as_ref(),
        );
        Ok(Json(response))
    }

    #[tool(
        name = "memory_delete",
        description = "Delete a memory inside the configured agent namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryDeleteArgs>()
    )]
    async fn memory_delete(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryDeleteResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryDeleteArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .delete_memory(&args)
            .await
            .map_err(map_store_error)?;
        self.write_auditor.audit_write(
            &request_context,
            "memory_delete",
            &response.id,
            self.memory_repository.as_ref(),
        );
        Ok(Json(response))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ElegyMemoryMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                    .with_title("Elegy Memory MCP")
                    .with_description("Reusable MCP memory tool surface for Elegy Memory."),
            )
            .with_instructions(
                "This server exposes read/write memory tools inside a caller-configured agent namespace.",
            )
    }
}
