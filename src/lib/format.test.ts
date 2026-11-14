import { describe, expect, it } from 'vitest';
import {
  fileName,
  formatCanvas,
  formatInt,
  formatPercent,
  formatSeconds,
  isCustomerId,
} from './format';

describe('format', () => {
  it('formats coverage ratios', () => {
    expect(formatPercent(0.8231)).toBe('82.3%');
    expect(formatPercent(0.8)).toBe('80.0%');
    expect(formatPercent(1)).toBe('100.0%');
  });

  it('formats integers and durations', () => {
    expect(formatInt(239796)).toBe('239,796');
    expect(formatInt(12)).toBe('12');
    expect(formatSeconds(1840)).toBe('1.8 s');
    expect(formatSeconds(0)).toBe('0.0 s');
    expect(formatCanvas([2048, 2048])).toBe('2048×2048');
  });

  it('takes the file name from Windows and POSIX paths', () => {
    expect(fileName('C:\\Users\\driver\\Downloads\\Porsche 992r GT3.psd')).toBe(
      'Porsche 992r GT3.psd',
    );
    expect(fileName('/home/driver/template.psd')).toBe('template.psd');
    expect(fileName('template.psd')).toBe('template.psd');
  });

  it('accepts only digits as a customer id', () => {
    expect(isCustomerId('')).toBe(true);
    expect(isCustomerId('123456')).toBe(true);
    expect(isCustomerId('12a')).toBe(false);
    expect(isCustomerId('1234567890123')).toBe(false);
    expect(isCustomerId(' 12')).toBe(false);
  });
});
