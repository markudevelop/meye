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
  pipeSetSchedule: (name: string, schedule: string) =>
    invoke("api_pipe_set_schedule", { name, schedule }) as Promise<void>,
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
  chat: (question: string) =>
    invoke("api_chat", { question }) as Promise<{
      answer: string;
      sources: { ts: string; app: string; text: string; frame_id: number | null }[];
    }>,
  openPipeDir: (name: string) => invoke("api_open_pipe_dir", { name }),
  perfStats: () => invoke("api_perf_stats") as Promise<any>,
  convoList: () => invoke("api_convo_list") as Promise<{ id: string; title: string; count: number; updated: number }[]>,
  convoRead: (id: string) => invoke("api_convo_read", { id }) as Promise<any[]>,
  convoAppend: (id: string, entry: any) => invoke("api_convo_append", { id, entry }) as Promise<void>,
  convoDelete: (id: string) => invoke("api_convo_delete", { id }) as Promise<void>,
  convoArchive: (id: string) => invoke("api_convo_archive", { id }) as Promise<void>,
  convoListArchived: () => invoke("api_convo_list_archived") as Promise<{ id: string; title: string; count: number; updated: number }[]>,
  convoUnarchive: (id: string) => invoke("api_convo_unarchive", { id }) as Promise<void>,
  getRecordArgs: () => invoke("api_get_record_args") as Promise<string[]>,
  setRecordArgs: (args: string[]) => invoke("api_set_record_args", { args }) as Promise<void>,
  retentionStatus: () => invoke("api_retention_status") as Promise<any>,
  retentionConfigure: (enabled: boolean, days: number, mode: string) =>
    invoke("api_retention_configure", { enabled, days, mode }) as Promise<any>,
  storagePreview: (olderThanDays: number) => invoke("api_storage_preview", { olderThanDays }) as Promise<any>,
  deleteRange: (start: string, end: string) => invoke("api_delete_range", { start, end }) as Promise<any>,
  getDiscreet: () => invoke("api_get_discreet") as Promise<boolean>,
  setDiscreet: (on: boolean) => invoke("api_set_discreet", { on }) as Promise<void>,
  // Remote viewing
  getRemoteEnabled: () => invoke("api_get_remote_enabled") as Promise<boolean>,
  setRemoteEnabled: (on: boolean) => invoke("api_set_remote_enabled", { on }) as Promise<void>,
  remotePairing: () => invoke("api_remote_pairing") as Promise<{ host: string; port: number; token: string; enabled: boolean }>,
  remoteLatest: (host: string, token: string, since: string) =>
    invoke("api_remote_latest", { host, token, since }) as Promise<any>,
  remoteAudio: (host: string, token: string, since: string, limit: number) =>
    invoke("api_remote_audio", { host, token, since, limit }) as Promise<any>,
  getObsidianVault: () => invoke("api_get_obsidian_vault") as Promise<string>,
  setObsidianVault: (path: string) => invoke("api_set_obsidian_vault", { path }) as Promise<void>,
  remoteFrame: (host: string, token: string, id: number) =>
    invoke("api_remote_frame", { host, token, id }) as Promise<string>,
  remoteComment: (context: string, question: string) =>
    invoke("api_remote_comment", { context, question }) as Promise<string>,
  parseVoiceCommand: (transcript: string) =>
    invoke("api_parse_voice_command", { transcript }) as Promise<{ action: string; arg: string } | null>,
};
