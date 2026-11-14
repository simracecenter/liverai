import type {
  AppInfo,
  CarDetail,
  CarSummary,
  ClaudeStatus,
  ProjectDetail,
  Settings,
} from '../lib/api';

export const cars: CarSummary[] = [
  {
    id: 'porsche992rgt3',
    name: 'Porsche 911 GT3 R (992)',
    canvas: [2048, 2048],
    panel_count: 2,
    confirmed_panel_count: 1,
    confirmed_coverage_ratio: 0.8231,
  },
  {
    id: 'ferrari296gt3',
    name: 'Ferrari 296 GT3',
    canvas: [2048, 2048],
    panel_count: 60,
    confirmed_panel_count: 40,
    confirmed_coverage_ratio: 0.8049,
  },
];

export const carDetail: CarDetail = {
  ...(cars[0] as CarSummary),
  atlas_schema: 'panels/0.1',
  paint_folder: 'porsche992rgt3',
  total_paintable_px: 1000,
  confirmed_paintable_px: 823,
  panels: [
    {
      id: 'p001',
      label: 'body_side_right',
      bbox: [772, 567, 2022, 1105],
      area_px: 363974,
      calibration_hex: '#EB2F2F',
      up_angle_deg: 180,
      forward: '-x',
      mirror_of: 'p002',
      confidence: 'confirmed',
      evidence_count: 3,
    },
    {
      id: 'p002',
      label: 'unknown_island',
      bbox: [0, 0, 10, 10],
      area_px: 100,
      calibration_hex: null,
      up_angle_deg: 0,
      forward: '-x',
      mirror_of: null,
      confidence: 'guess',
      evidence_count: 0,
    },
  ],
};

export const settings: Settings = {
  paint_folder: 'C:\\Users\\driver\\Documents\\iRacing\\paint',
  customer_id: '123456',
  model: 'claude-opus-5-5',
  effort: 'high',
  ask_before_install: true,
  ask_before_input: true,
  backup_before_install: true,
};

export const project: ProjectDetail = {
  schema: 'project/1',
  id: 'works-livery',
  name: 'Works livery',
  car_id: 'porsche992rgt3',
  customer_id: '123456',
  paint_type: 'driver',
  car_number: 'sim',
  template_path: null,
  created_at: '2026-11-14T09:30:00Z',
  updated_at: '2026-11-14T09:30:00Z',
  revision_count: 0,
  brief: 'Apex Orange body.',
  folder: 'C:\\Users\\driver\\Documents\\Livery AI\\works-livery',
};

export const appInfo: AppInfo = { app_version: '0.0.1', engine_version: '0.0.1' };

export const claudeReady: ClaudeStatus = {
  cli_found: true,
  version: '2.1.325',
  sign_in: { state: 'signed_in', method: 'subscription' },
};
