import mark from '../assets/simracecenter-mark-white.svg';
import type { ClaudeStatus } from '../lib/api';
import { connectionLabel } from '../lib/claude';
import { type NavSection, routeHref } from '../lib/router';
import type { Loadable } from '../lib/useLoad';

const NAV: { section: NavSection; label: string; href: string }[] = [
  { section: 'garage', label: 'Garage', href: routeHref({ name: 'garage' }) },
  { section: 'cars', label: 'Cars', href: routeHref({ name: 'cars', id: null }) },
  { section: 'settings', label: 'Settings', href: routeHref({ name: 'settings' }) },
];

export function AppHeader({
  section,
  claude,
}: {
  section: NavSection;
  claude: Loadable<ClaudeStatus>;
}) {
  const connection = connectionLabel(claude);
  const dot =
    connection.tone === 'data'
      ? 'chip__dot chip__dot--ok'
      : connection.tone === 'caution'
        ? 'chip__dot chip__dot--caution'
        : 'chip__dot';
  return (
    <header className="app-header">
      <div className="app-header__brand">
        <img src={mark} alt="Sim RaceCenter mark" />
        <span>LIVERY AI</span>
      </div>
      <nav className="app-header__nav" aria-label="Main">
        {NAV.map((item) => (
          <a
            key={item.section}
            className="app-header__link"
            href={item.href}
            aria-current={item.section === section ? 'page' : undefined}
          >
            {item.label}
          </a>
        ))}
      </nav>
      <a className="chip" href={routeHref({ name: 'settings' })} title="Claude connection">
        <span className={dot} />
        Claude · {connection.text}
      </a>
    </header>
  );
}
