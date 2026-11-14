import { describe, expect, it } from 'vitest';
import { navSection, parseRoute, type Route, routeHref } from './router';

describe('parseRoute', () => {
  it('defaults to the garage', () => {
    for (const hash of ['', '#', '#/', '#/garage', '#/unknown', '#/studio']) {
      expect(parseRoute(hash)).toEqual({ name: 'garage' });
    }
  });

  it('parses each screen', () => {
    expect(parseRoute('#/new')).toEqual({ name: 'new' });
    expect(parseRoute('#/studio/works-livery')).toEqual({ name: 'studio', id: 'works-livery' });
    expect(parseRoute('#/studio/works-livery/showroom')).toEqual({
      name: 'showroom',
      id: 'works-livery',
    });
    expect(parseRoute('#/studio/works-livery/export')).toEqual({
      name: 'export',
      id: 'works-livery',
    });
    expect(parseRoute('#/cars')).toEqual({ name: 'cars', id: null });
    expect(parseRoute('#/cars/mx5_mx52016')).toEqual({ name: 'cars', id: 'mx5_mx52016' });
    expect(parseRoute('#/settings')).toEqual({ name: 'settings' });
  });

  it('survives malformed escapes', () => {
    expect(parseRoute('#/studio/%E0%A4%A')).toEqual({ name: 'studio', id: '%E0%A4%A' });
  });
});

describe('routeHref', () => {
  it('round-trips every route', () => {
    const routes: Route[] = [
      { name: 'garage' },
      { name: 'new' },
      { name: 'studio', id: 'a-b' },
      { name: 'showroom', id: 'a-b' },
      { name: 'export', id: 'a-b' },
      { name: 'cars', id: null },
      { name: 'cars', id: 'ferrari296gt3' },
      { name: 'settings' },
    ];
    for (const route of routes) {
      expect(parseRoute(routeHref(route))).toEqual(route);
    }
  });

  it('escapes ids', () => {
    expect(routeHref({ name: 'studio', id: 'a/b c' })).toBe('#/studio/a%2Fb%20c');
    expect(parseRoute('#/studio/a%2Fb%20c')).toEqual({ name: 'studio', id: 'a/b c' });
  });
});

describe('navSection', () => {
  it('keeps project screens under Garage', () => {
    expect(navSection({ name: 'new' })).toBe('garage');
    expect(navSection({ name: 'studio', id: 'x' })).toBe('garage');
    expect(navSection({ name: 'export', id: 'x' })).toBe('garage');
    expect(navSection({ name: 'cars', id: null })).toBe('cars');
    expect(navSection({ name: 'settings' })).toBe('settings');
  });
});
