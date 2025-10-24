export type McpStdioTransport = {
  type: "stdio";
  command: string;
  args: string[];
  env?: Record<string, string> | null;
  env_vars?: string[] | null;
  cwd?: string | null;
};

export type McpStreamableHttpTransport = {
  type: "streamable_http";
  url: string;
  bearer_token_env_var?: string | null;
  http_headers?: Record<string, string> | null;
  env_http_headers?: Record<string, string> | null;
};

export type McpTransportSummary = McpStdioTransport | McpStreamableHttpTransport;

export type McpServerSummary = {
  name: string;
  enabled: boolean;
  transport: McpTransportSummary;
  startup_timeout_sec?: number | null;
  tool_timeout_sec?: number | null;
  auth_status?: string;
};

export type McpServerDetails = {
  name: string;
  enabled: boolean;
  transport: McpTransportSummary;
  enabled_tools?: string[] | null;
  disabled_tools?: string[] | null;
  startup_timeout_sec?: number | null;
  tool_timeout_sec?: number | null;
};

export type McpAddTransportOptions =
  | {
      type: "stdio";
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  | {
      type: "streamable_http";
      url: string;
      bearerTokenEnvVar?: string;
    };

export type McpMutableFields = {
  enabled?: boolean;
  enabledTools?: string[] | null;
  disabledTools?: string[] | null;
  startupTimeoutSec?: number | null;
  toolTimeoutSec?: number | null;
};

export type EnableOnceOptions = {
  enabledTools?: string[] | null;
  disabledTools?: string[] | null;
};
