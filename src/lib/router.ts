// Hash router: the whole app is one window with a handful of screens.
import { useSyncExternalStore } from 'react';

export type Route =
  | { name: 'garage' }
  | { name: 'new' }
  | { name: 'studio'; id: string }
  | { name: 'showroom'; id: string }
  | { name: 'export'; id: string }
  | { name: 'cars'; id: string | null }
  | { name: 'settings' };

export type NavSection = 'garage' | 'cars' | 'settings';

export function parseRoute(hash: string): Route {
  const parts = hash
    .replace(/^#/, '')
    .split('/')
    .filter(Boolean)
    .map((part) => {
      try {
        return decodeURIComponent(part);
      } catch {
        return part;
      }
    });
  const [head, id, sub] = parts;
  switch (head) {
    case 'new':
      return { name: 'new' };
    case 'studio':
      if (!id) return { name: 'garage' };
      if (sub === 'showroom') return { name: 'showroom', id };
      if (sub === 'export') return { name: 'export', id };
      return { name: 'studio', id };
    case 'cars':
      return { name: 'cars', id: id ?? null };
    case 'settings':
      return { name: 'settings' };
    default:
      return { name: 'garage' };
  }
}

export function routeHref(route: Route): string {
  switch (route.name) {
    case 'garage':
      return '#/garage';
    case 'new':
      return '#/new';
    case 'studio':
      return `#/studio/${encodeURIComponent(route.id)}`;
    case 'showroom':
      return `#/studio/${encodeURIComponent(route.id)}/showroom`;
    case 'export':
      return `#/studio/${encodeURIComponent(route.id)}/export`;
    case 'cars':
      return route.id ? `#/cars/${encodeURIComponent(route.id)}` : '#/cars';
    case 'settings':
      return '#/settings';
  }
}

/** Which top-nav item a route belongs to. */
export function navSection(route: Route): NavSection {
  if (route.name === 'cars') return 'cars';
  if (route.name === 'settings') return 'settings';
  return 'garage';
}

export function navigate(route: Route): void {
  window.location.hash = routeHref(route);
}

function subscribe(onChange: () => void): () => void {
  window.addEventListener('hashchange', onChange);
  return () => {
    window.removeEventListener('hashchange', onChange);
  };
}

/** The current hash; parse it with `parseRoute`. */
export function useHash(): string {
  return useSyncExternalStore(subscribe, () => window.location.hash);
}
