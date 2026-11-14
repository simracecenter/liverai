import { describe, expect, it } from 'vitest';
import type { ClaudeStatus } from './api';
import { cliLabel, connectionLabel, isSubscriptionReady, signInLabel, testLabel } from './claude';

const status = (over: Partial<ClaudeStatus> = {}): ClaudeStatus => ({
  cli_found: true,
  version: '2.1.325',
  sign_in: { state: 'signed_in', method: 'subscription' },
  ...over,
});

describe('claude labels', () => {
  it('reports a ready subscription', () => {
    const s = status();
    expect(cliLabel(s)).toEqual({ text: 'DETECTED · 2.1.325', tone: 'data' });
    expect(signInLabel(s)).toEqual({ text: 'ACTIVE · SUBSCRIPTION', tone: 'data' });
    expect(isSubscriptionReady(s)).toBe(true);
    expect(connectionLabel({ status: 'ready', data: s }).text).toBe('SIGNED IN');
  });

  it('reports a missing CLI as caution and sign-in as unknown', () => {
    const s = status({ cli_found: false, version: null, sign_in: { state: 'unknown', method: null } });
    expect(cliLabel(s)).toEqual({ text: 'NOT FOUND', tone: 'caution' });
    expect(signInLabel(s)).toEqual({ text: 'UNKNOWN', tone: 'dim' });
    expect(connectionLabel({ status: 'ready', data: s })).toEqual({
      text: 'NOT FOUND',
      tone: 'caution',
    });
  });

  it('never calls a non-subscription sign-in ready', () => {
    const s = status({ sign_in: { state: 'signed_in', method: 'api_key' } });
    expect(isSubscriptionReady(s)).toBe(false);
    expect(signInLabel(s)).toEqual({ text: 'ACTIVE · API KEY', tone: 'caution' });
    expect(connectionLabel({ status: 'ready', data: s }).text).toBe('NOT ON SUBSCRIPTION');
  });

  it('reports signed out and unknown honestly', () => {
    const out = status({ sign_in: { state: 'signed_out', method: null } });
    expect(signInLabel(out).text).toBe('NOT SIGNED IN');
    expect(connectionLabel({ status: 'ready', data: out }).tone).toBe('caution');
    const unknown = status({ sign_in: { state: 'unknown', method: null } });
    expect(signInLabel(unknown)).toEqual({ text: 'UNKNOWN', tone: 'dim' });
    expect(connectionLabel({ status: 'ready', data: unknown }).text).toBe('UNKNOWN');
    expect(connectionLabel({ status: 'loading' }).text).toBe('CHECKING');
    expect(connectionLabel({ status: 'error', error: 'x' }).text).toBe('UNKNOWN');
  });

  it('labels test results', () => {
    expect(testLabel(null, false)).toEqual({ text: 'NOT RUN', tone: 'dim' });
    expect(testLabel(null, true).text).toBe('RUNNING');
    expect(testLabel({ ok: true, elapsed_ms: 1840, reason: null, exit_code: 0 }, false)).toEqual({
      text: 'PASS · 1.8 S',
      tone: 'data',
    });
    expect(
      testLabel({ ok: false, elapsed_ms: 500, reason: 'exit_code', exit_code: 1 }, false),
    ).toEqual({ text: 'FAIL · EXIT 1 · 0.5 S', tone: 'critical' });
    expect(
      testLabel({ ok: false, elapsed_ms: 90000, reason: 'timeout', exit_code: null }, false).text,
    ).toBe('FAIL · TIMED OUT · 90.0 S');
  });
});
