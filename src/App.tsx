import { AppHeader } from './components/AppHeader';
import { api } from './lib/api';
import { navSection, parseRoute, useHash } from './lib/router';
import { useLoad } from './lib/useLoad';
import { Cars } from './screens/Cars';
import { ExportScreen } from './screens/Export';
import { Garage } from './screens/Garage';
import { NewLivery } from './screens/NewLivery';
import { Settings } from './screens/Settings';
import { Showroom } from './screens/Showroom';
import { Studio } from './screens/Studio';

export function App() {
  const route = parseRoute(useHash());
  const claude = useLoad(api.claudeStatus, 'claude');

  return (
    <div className="app">
      <AppHeader section={navSection(route)} claude={claude.state} />
      {route.name === 'garage' && <Garage />}
      {route.name === 'new' && <NewLivery claude={claude.state} />}
      {route.name === 'studio' && <Studio key={route.id} id={route.id} />}
      {route.name === 'showroom' && <Showroom key={route.id} id={route.id} />}
      {route.name === 'export' && <ExportScreen key={route.id} id={route.id} />}
      {route.name === 'cars' && <Cars selectedId={route.id} />}
      {route.name === 'settings' && (
        <Settings claude={claude.state} onRefreshClaude={claude.reload} />
      )}
    </div>
  );
}
