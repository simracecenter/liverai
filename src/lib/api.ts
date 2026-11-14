// Typed wrappers over the Tauri core commands. Field names match the Rust
// structs (snake_case).
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export interface AppInfo {
  app_version: string;
  engine_version: string;
}

export type Effort = 'low' | 'medium' | 'high';

export interface Settings {
  paint_folder: string;
  customer_id: string;
  model: string;
  effort: Effort;
  ask_before_install: boolean;
  ask_before_input: boolean;
  backup_before_install: boolean;
}

export interface CarSummary {
  id: string;
  name: string;
  canvas: [number, number];
  panel_count: number;
  confirmed_panel_count: number;
  confirmed_coverage_ratio: number;
}

export interface Panel {
  id: string;
  label: string | null;
  bbox: [number, number, number, number];
  area_px: number;
  calibration_hex: string | null;
  up_angle_deg: number | null;
  forward: string | null;
  mirror_of: string | null;
  confidence: string;
  evidence_count: number;
}

export interface CarDetail extends CarSummary {
  atlas_schema: string;
  paint_folder: string;
  total_paintable_px: number;
  confirmed_paintable_px: number;
  panels: Panel[];
}

export type PaintType = 'driver' | 'team';
export type CarNumberMode = 'sim' | 'custom';

export interface ProjectInput {
  name: string;
  car_id: string;
  template_path: string | null;
  customer_id: string;
  brief: string;
  paint_type: PaintType;
  car_number: CarNumberMode;
}

export interface ProjectSummary {
  schema: string;
  id: string;
  name: string;
  car_id: string;
  customer_id: string;
  paint_type: PaintType;
  car_number: CarNumberMode;
  template_path: string | null;
  created_at: string;
  updated_at: string;
  revision_count: number;
}

export interface ProjectDetail extends ProjectSummary {
  brief: string;
  folder: string;
}

export type SignInState = 'signed_in' | 'signed_out' | 'unknown';
export type AuthMethod =
  | 'subscription'
  | 'oauth_token'
  | 'api_key'
  | 'api_key_helper'
  | 'third_party';

export interface ClaudeStatus {
  cli_found: boolean;
  version: string | null;
  sign_in: { state: SignInState; method: AuthMethod | null };
}

export type ClaudeTestReason =
  | 'cli_not_found'
  | 'timeout'
  | 'spawn_failed'
  | 'exit_code'
  | 'bad_output'
  | 'reported_error';

export interface ClaudeTest {
  ok: boolean;
  elapsed_ms: number;
  reason: ClaudeTestReason | null;
  exit_code: number | null;
}

export interface SignInLaunch {
  launched: boolean;
  command: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  settingsGet: () => invoke<Settings>('settings_get'),
  settingsSet: (settings: Settings) => invoke<Settings>('settings_set', { settings }),
  carsList: () => invoke<CarSummary[]>('cars_list'),
  carGet: (id: string) => invoke<CarDetail>('car_get', { id }),
  projectsList: () => invoke<ProjectSummary[]>('projects_list'),
  projectCreate: (input: ProjectInput) => invoke<ProjectDetail>('project_create', { input }),
  projectGet: (id: string) => invoke<ProjectDetail>('project_get', { id }),
  claudeStatus: () => invoke<ClaudeStatus>('claude_status'),
  claudeOpenSignin: () => invoke<SignInLaunch>('claude_open_signin'),
  claudeTest: () => invoke<ClaudeTest>('claude_test'),

  /** Native picker for an iRacing template. Resolves to null when cancelled. */
  pickTemplate: async (): Promise<string | null> => {
    const picked = await open({
      title: 'Select the iRacing template',
      multiple: false,
      directory: false,
      filters: [{ name: 'Photoshop template', extensions: ['psd'] }],
    });
    return typeof picked === 'string' ? picked : null;
  },

  /** Native folder picker. Resolves to null when cancelled. */
  pickFolder: async (title: string, defaultPath?: string): Promise<string | null> => {
    const picked = await open({ title, multiple: false, directory: true, defaultPath });
    return typeof picked === 'string' ? picked : null;
  },
};

/** Short message from a core error (`{ code, message }`) or anything thrown. */
export function errorMessage(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err && typeof err === 'object' && 'message' in err && typeof err.message === 'string') {
    return err.message;
  }
  return 'Unexpected error.';
}
