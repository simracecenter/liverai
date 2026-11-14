// Display mapping for the Claude connection state.
import type { ClaudeStatus, ClaudeTest } from './api';
import { formatSeconds } from './format';
import type { Loadable } from './useLoad';

export type Tone = 'data' | 'dim' | 'caution' | 'critical';

export interface StatusLabel {
  text: string;
  tone: Tone;
}

export function cliLabel(status: ClaudeStatus): StatusLabel {
  if (!status.cli_found) return { text: 'NOT FOUND', tone: 'caution' };
  return {
    text: status.version ? `DETECTED · ${status.version}` : 'DETECTED · VERSION UNKNOWN',
    tone: 'data',
  };
}

export function signInLabel(status: ClaudeStatus): StatusLabel {
  if (!status.cli_found) return { text: 'UNKNOWN', tone: 'dim' };
  switch (status.sign_in.state) {
    case 'signed_out':
      return { text: 'NOT SIGNED IN', tone: 'caution' };
    case 'unknown':
      return { text: 'UNKNOWN', tone: 'dim' };
    case 'signed_in':
      switch (status.sign_in.method) {
        case 'subscription':
          return { text: 'ACTIVE · SUBSCRIPTION', tone: 'data' };
        case 'oauth_token':
          return { text: 'ACTIVE · OAUTH TOKEN', tone: 'caution' };
        case 'api_key':
        case 'api_key_helper':
          return { text: 'ACTIVE · API KEY', tone: 'caution' };
        case 'third_party':
          return { text: 'ACTIVE · THIRD-PARTY PROVIDER', tone: 'caution' };
        case null:
          return { text: 'ACTIVE', tone: 'data' };
      }
  }
}

/** True when Claude Code is signed in with a Claude subscription. */
export function isSubscriptionReady(status: ClaudeStatus): boolean {
  return (
    status.cli_found &&
    status.sign_in.state === 'signed_in' &&
    status.sign_in.method === 'subscription'
  );
}

/** One-word connection state for the header chip and pre-flight. */
export function connectionLabel(state: Loadable<ClaudeStatus>): StatusLabel {
  if (state.status === 'loading') return { text: 'CHECKING', tone: 'dim' };
  if (state.status === 'error') return { text: 'UNKNOWN', tone: 'dim' };
  const status = state.data;
  if (!status.cli_found) return { text: 'NOT FOUND', tone: 'caution' };
  if (isSubscriptionReady(status)) return { text: 'SIGNED IN', tone: 'data' };
  if (status.sign_in.state === 'signed_out') return { text: 'NOT SIGNED IN', tone: 'caution' };
  if (status.sign_in.state === 'signed_in') return { text: 'NOT ON SUBSCRIPTION', tone: 'caution' };
  return { text: 'UNKNOWN', tone: 'dim' };
}

export function testLabel(test: ClaudeTest | null, running: boolean): StatusLabel {
  if (running) return { text: 'RUNNING', tone: 'dim' };
  if (!test) return { text: 'NOT RUN', tone: 'dim' };
  const elapsed = formatSeconds(test.elapsed_ms).toUpperCase();
  if (test.ok) return { text: `PASS · ${elapsed}`, tone: 'data' };
  const reasons: Record<NonNullable<ClaudeTest['reason']>, string> = {
    cli_not_found: 'CLAUDE CODE NOT FOUND',
    timeout: 'TIMED OUT',
    spawn_failed: 'COULD NOT START',
    exit_code: test.exit_code === null ? 'STOPPED' : `EXIT ${test.exit_code}`,
    bad_output: 'UNEXPECTED OUTPUT',
    reported_error: 'CLAUDE CODE REPORTED AN ERROR',
  };
  const reason = test.reason ? reasons[test.reason] : 'FAILED';
  return { text: `FAIL · ${reason} · ${elapsed}`, tone: 'critical' };
}
