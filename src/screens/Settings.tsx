import './Settings.css';

import { type SyntheticEvent, useState } from 'react';
import { Alert, Load, Row } from '../components/ui';
import {
  api,
  type AppInfo,
  type ClaudeStatus,
  type ClaudeTest,
  type Effort,
  errorMessage,
  type Settings as SettingsData,
} from '../lib/api';
import { cliLabel, signInLabel, testLabel } from '../lib/claude';
import { isCustomerId } from '../lib/format';
import { type Loadable, useLoad } from '../lib/useLoad';

/** Model ids offered in Settings. The id is what gets stored. */
const MODELS: { id: string; label: string }[] = [
  { id: 'claude-opus-5-5', label: 'Claude Opus 5.5 (default)' },
  { id: 'claude-fable-5-2', label: 'Claude Fable 5.2' },
  { id: 'claude-sonnet-5-5', label: 'Claude Sonnet 5.5' },
];

const EFFORTS: { id: Effort; label: string }[] = [
  { id: 'high', label: 'High' },
  { id: 'medium', label: 'Medium' },
  { id: 'low', label: 'Low' },
];

const PERMISSIONS: {
  key: 'ask_before_install' | 'ask_before_input' | 'backup_before_install';
  label: string;
}[] = [
  { key: 'ask_before_install', label: 'Ask before writing to the paint folder' },
  { key: 'ask_before_input', label: 'Ask before sending reload and camera input to iRacing' },
  { key: 'backup_before_install', label: 'Back up existing paint files before each install' },
];

const SECTIONS = [
  { id: 'claude', label: 'Claude' },
  { id: 'iracing', label: 'iRacing' },
  { id: 'permissions', label: 'Permissions' },
  { id: 'about', label: 'About' },
];

interface SettingsLoad {
  settings: SettingsData;
  info: AppInfo;
}

async function loadSettings(): Promise<SettingsLoad> {
  const [settings, info] = await Promise.all([api.settingsGet(), api.appInfo()]);
  return { settings, info };
}

export function Settings({
  claude,
  onRefreshClaude,
}: {
  claude: Loadable<ClaudeStatus>;
  onRefreshClaude: () => void;
}) {
  const { state } = useLoad(loadSettings, 'settings');
  return (
    <div className="screen settings">
      <nav className="settings__nav" aria-label="Settings sections">
        <span className="section-title settings__nav-title">Settings</span>
        {SECTIONS.map((section) => (
          <button
            key={section.id}
            type="button"
            className="settings__nav-link"
            onClick={() => {
              document.getElementById(section.id)?.scrollIntoView({ block: 'start' });
            }}
          >
            {section.label}
          </button>
        ))}
      </nav>
      <main className="settings__main">
        <Load state={state}>
          {(loaded) => (
            <SettingsForm loaded={loaded} claude={claude} onRefreshClaude={onRefreshClaude} />
          )}
        </Load>
      </main>
    </div>
  );
}

function SettingsForm({
  loaded,
  claude,
  onRefreshClaude,
}: {
  loaded: SettingsLoad;
  claude: Loadable<ClaudeStatus>;
  onRefreshClaude: () => void;
}) {
  const [saved, setSaved] = useState(loaded.settings);
  const [draft, setDraft] = useState(loaded.settings);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [test, setTest] = useState<ClaudeTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const customerIdValid = isCustomerId(draft.customer_id);
  const canSave = dirty && customerIdValid && draft.paint_folder.trim() !== '' && !saving;
  const knownModel = MODELS.some((m) => m.id === draft.model);

  const patch = (change: Partial<SettingsData>) => {
    setDraft((d) => ({ ...d, ...change }));
  };

  const save = (event: SyntheticEvent) => {
    event.preventDefault();
    if (!canSave) return;
    setSaving(true);
    setError(null);
    api
      .settingsSet(draft)
      .then(
        (stored) => {
          setSaved(stored);
          setDraft(stored);
        },
        (err: unknown) => {
          setError(errorMessage(err));
        },
      )
      .finally(() => {
        setSaving(false);
      });
  };

  const browsePaintFolder = () => {
    api.pickFolder('Select the iRacing paint folder', draft.paint_folder).then(
      (picked) => {
        if (picked) patch({ paint_folder: picked });
      },
      (err: unknown) => {
        setError(errorMessage(err));
      },
    );
  };

  const openSignIn = () => {
    setNotice(null);
    api.claudeOpenSignin().then(
      (launch) => {
        setNotice(
          `Sign-in opened in a terminal (${launch.command}). Finish it there, then refresh the status.`,
        );
      },
      (err: unknown) => {
        setNotice(errorMessage(err));
      },
    );
  };

  const runTest = () => {
    setTesting(true);
    setNotice(null);
    api
      .claudeTest()
      .then(setTest, (err: unknown) => {
        setNotice(errorMessage(err));
      })
      .finally(() => {
        setTesting(false);
      });
  };

  const status = claude.status === 'ready' ? claude.data : null;
  const cli = status ? cliLabel(status) : null;
  const signIn = status ? signInLabel(status) : null;
  const lastTest = testLabel(test, testing);
  const pending = claude.status === 'loading' ? 'CHECKING' : 'UNKNOWN';

  return (
    <form className="settings__form" onSubmit={save}>
      <section id="claude" className="settings__section">
        <div className="settings__heading">
          <h1 className="settings__title">Claude connection</h1>
          <span className="dim">
            The designer runs on your own Claude account, through Claude Code on this PC.
          </span>
        </div>

        <div className="card settings__claude">
          <h2 className="settings__card-title">Claude subscription</h2>
          <span className="note">
            Uses Claude Code on this PC, signed in with your Pro, Max, Team or Enterprise plan.
            Livery AI never sees or stores your sign-in.
          </span>
          <div className="rows">
            <Row label="Claude Code" value={cli?.text ?? pending} tone={cli?.tone ?? 'dim'} />
            <Row label="Sign-in" value={signIn?.text ?? pending} tone={signIn?.tone ?? 'dim'} />
            <Row label="Last test" value={lastTest.text} tone={lastTest.tone} />
          </div>
          {claude.status === 'error' ? <Alert>{claude.error}</Alert> : null}
          {status && !status.cli_found ? (
            <span className="note">
              Claude Code was not found on PATH. Install Claude Code, sign in, then refresh.
            </span>
          ) : null}
          {status?.sign_in.state === 'signed_in' && status.sign_in.method !== 'subscription' ? (
            <span className="note caution">
              Claude Code is not signed in with a Claude subscription. Livery AI supports
              subscription sign-in only.
            </span>
          ) : null}
          {notice ? (
            <span className="note" role="status">
              {notice}
            </span>
          ) : null}
          <div className="settings__buttons">
            <button type="button" className="btn" onClick={openSignIn}>
              Open Claude sign-in
            </button>
            <button type="button" className="btn" onClick={runTest} disabled={testing}>
              {testing ? 'Testing' : 'Test connection'}
            </button>
            <button type="button" className="btn btn--quiet" onClick={onRefreshClaude}>
              Refresh status
            </button>
          </div>
          <span className="meta">POWERED BY CLAUDE</span>
        </div>

        <div className="card settings__model">
          <div className="field settings__grow">
            <label className="field__label" htmlFor="set-model">
              Model
            </label>
            <select
              id="set-model"
              className="input"
              value={draft.model}
              onChange={(e) => {
                patch({ model: e.target.value });
              }}
            >
              {MODELS.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.label}
                </option>
              ))}
              {knownModel ? null : <option value={draft.model}>{draft.model}</option>}
            </select>
            <span className="meta">{draft.model}</span>
          </div>
          <div className="field settings__narrow">
            <label className="field__label" htmlFor="set-effort">
              Effort
            </label>
            <select
              id="set-effort"
              className="input"
              value={draft.effort}
              onChange={(e) => {
                const effort = EFFORTS.find((x) => x.id === e.target.value);
                if (effort) patch({ effort: effort.id });
              }}
            >
              {EFFORTS.map((effort) => (
                <option key={effort.id} value={effort.id}>
                  {effort.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <span className="note">Test connection uses the saved model.</span>
      </section>

      <section id="iracing" className="settings__section">
        <h2 className="settings__title">iRacing</h2>
        <div className="card">
          <div className="form-row">
            <div className="field settings__wide">
              <label className="field__label" htmlFor="set-paint">
                Paint folder
              </label>
              <div className="settings__folder">
                <input
                  id="set-paint"
                  className="input input--mono"
                  type="text"
                  spellCheck={false}
                  value={draft.paint_folder}
                  onChange={(e) => {
                    patch({ paint_folder: e.target.value });
                  }}
                />
                <button type="button" className="btn" onClick={browsePaintFolder}>
                  Browse
                </button>
              </div>
              <span className="meta">DEFAULT %USERPROFILE%\Documents\iRacing\paint</span>
            </div>
            <div className="field settings__narrow">
              <label className="field__label" htmlFor="set-cust">
                Customer ID
              </label>
              <input
                id="set-cust"
                className="input input--data"
                type="text"
                inputMode="numeric"
                autoComplete="off"
                aria-invalid={!customerIdValid}
                value={draft.customer_id}
                onChange={(e) => {
                  patch({ customer_id: e.target.value.trim() });
                }}
              />
              <span className={`meta ${customerIdValid ? '' : 'caution'}`}>
                {customerIdValid ? (draft.customer_id ? 'SET MANUALLY' : 'NOT SET') : 'DIGITS ONLY'}
              </span>
            </div>
          </div>
          <span className="note">
            Paint Shop detection and showroom capture are not available in this build.
          </span>
        </div>
      </section>

      <section id="permissions" className="settings__section">
        <h2 className="settings__title">Permissions</h2>
        <div className="card settings__toggles">
          {PERMISSIONS.map((permission) => (
            <label key={permission.key} className="settings__toggle">
              <span>{permission.label}</span>
              <input
                type="checkbox"
                checked={draft[permission.key]}
                onChange={(e) => {
                  patch({ [permission.key]: e.target.checked });
                }}
              />
            </label>
          ))}
        </div>
      </section>

      <section id="about" className="settings__section">
        <h2 className="settings__title">About</h2>
        <div className="card">
          <div className="rows rows--flush">
            <Row label="Livery AI" value={loaded.info.app_version} />
            <Row label="Render engine" value={loaded.info.engine_version} />
            <Row label="Licence" value="GPL-3.0-or-later" tone="dim" plain />
            <Row label="Designer" value="Powered by Claude" tone="dim" plain />
          </div>
          <span className="note">
            Livery AI is an open-source Sim RaceCenter tool. iRacing templates are not included or
            redistributed.
          </span>
        </div>
      </section>

      <div className="settings__save">
        {error ? <Alert>{error}</Alert> : null}
        <span className="meta">{dirty ? 'UNSAVED CHANGES' : 'SAVED'}</span>
        <button type="submit" className="btn btn--primary" disabled={!canSave}>
          {saving ? 'Saving' : 'Save settings'}
        </button>
      </div>
    </form>
  );
}
