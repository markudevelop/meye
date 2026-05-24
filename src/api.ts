import { invoke } from "@tauri-apps/api/core";

export type SearchParams = {
  q?: string;
  content_type?: string;
  app_name?: string;
  window_name?: string;
  start_time?: string;
  end_time?: string;
  limit?: number;
  offset?: number;
};

export const api = {
  getState: () => invoke("get_state") as Promise<{ installed: boolean; pinned: boolean; loaded: boolean }>,
  getHealth: () => invoke("get_health") as Promise<any | null>,
  getPermissions: () => invoke("get_permissions") as Promise<{ waiting: string[] }>,
  tailLogs: (lines: number) => invoke("tail_logs", { lines }) as Promise<string>,
  setup: () => invoke("setup"),
  start: () => invoke("start"),
  stop: () => invoke("stop"),
  restart: () => invoke("restart"),
  recheck: () => invoke("recheck"),
  update: () => invoke("update_screenpipe"),
  openData: () => invoke("open_data_dir"),
  openLogs: () => invoke("open_logs"),
  openSettings: (pane: string) => invoke("open_settings", { pane }),
  search: (params: SearchParams) => invoke("api_search", { params }) as Promise<any>,
  audioDevices: () => invoke("api_audio_devices") as Promise<any>,
  monitors: () => invoke("api_monitors") as Promise<any>,
  frameOcr: (id: number) => invoke("api_frame_ocr", { id }) as Promise<any>,
  audioStart: () => invoke("api_audio_start"),
  audioStop: () => invoke("api_audio_stop"),
  rawSql: (query: string) => invoke("api_raw_sql", { query }) as Promise<any>,
  addTags: (kind: string, id: number, tags: string[]) => invoke("api_add_tags", { kind, id, tags }),
  pipeList: () => invoke("api_pipe_list") as Promise<any>,
  pipeRun: (name: string) => invoke("api_pipe_run", { name }) as Promise<string>,
  pipeEnable: (name: string) => invoke("api_pipe_enable", { name }) as Promise<string>,
  pipeDisable: (name: string) => invoke("api_pipe_disable", { name }) as Promise<string>,
  pipeLogs: (name: string) => invoke("api_pipe_logs", { name }) as Promise<string>,
  modelsList: () => invoke("api_models_list") as Promise<any>,
  modelsCreate: (a: { id: string; provider: string; model: string; url?: string; apiKey?: string; setDefault: boolean }) =>
    invoke("api_models_create", {
      id: a.id,
      provider: a.provider,
      model: a.model,
      url: a.url,
      apiKey: a.apiKey,
      setDefault: a.setDefault,
    }) as Promise<string>,
  modelsSetDefault: (id: string) => invoke("api_models_set_default", { id }) as Promise<string>,
  modelsDelete: (id: string) => invoke("api_models_delete", { id }) as Promise<string>,
  pipeSetPreset: (name: string, presets: string[]) => invoke("api_pipe_set_preset", { name, presets }) as Promise<string>,
  pipeConfigRead: (name: string) => invoke("api_pipe_config_read", { name }) as Promise<string>,
  pipeConfigWrite: (name: string, content: string) => invoke("api_pipe_config_write", { name, content }) as Promise<void>,
  registrySearch: (query: string) => invoke("api_registry_search", { query }) as Promise<any>,
  registryInfo: (slug: string) => invoke("api_registry_info", { slug }) as Promise<string>,
  registryInstall: (source: string) => invoke("api_registry_install", { source }) as Promise<string>,
  pipeDelete: (name: string) => invoke("api_pipe_delete", { name }) as Promise<string>,
  chat: (question: string) => invoke("api_chat", { question }) as Promise<string>,
};
