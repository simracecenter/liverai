import './Garage.css';

import { useState } from 'react';
import { Empty, Load } from '../components/ui';
import { api, type CarSummary, type ProjectSummary } from '../lib/api';
import { routeHref } from '../lib/router';
import { useLoad } from '../lib/useLoad';

interface GarageData {
  projects: ProjectSummary[];
  cars: CarSummary[];
}

async function loadGarage(): Promise<GarageData> {
  const [projects, cars] = await Promise.all([api.projectsList(), api.carsList()]);
  return { projects, cars };
}

export function Garage() {
  const { state } = useLoad(loadGarage, 'garage');
  const [search, setSearch] = useState('');
  const [carFilter, setCarFilter] = useState('');

  const data = state.status === 'ready' ? state.data : null;
  const carName = (id: string) => data?.cars.find((c) => c.id === id)?.name ?? id;

  return (
    <main className="screen garage">
      <div className="garage__bar">
        <div className="garage__heading">
          <h1 className="page-title">Garage</h1>
          <span className="meta">
            LIVERIES <span className="data">{data ? data.projects.length : '—'}</span> · CARS
            CALIBRATED <span className="data">{data ? data.cars.length : '—'}</span>
          </span>
        </div>
        <label className="visually-hidden" htmlFor="garage-search">
          Search liveries
        </label>
        <input
          id="garage-search"
          className="input input--surface garage__search"
          type="search"
          placeholder="Search liveries"
          value={search}
          onChange={(e) => {
            setSearch(e.target.value);
          }}
        />
        <label className="visually-hidden" htmlFor="garage-car">
          Filter by car
        </label>
        <select
          id="garage-car"
          className="input input--surface garage__filter"
          value={carFilter}
          onChange={(e) => {
            setCarFilter(e.target.value);
          }}
        >
          <option value="">All cars</option>
          {data?.cars.map((car) => (
            <option key={car.id} value={car.id}>
              {car.name}
            </option>
          ))}
        </select>
        <a className="btn btn--primary" href={routeHref({ name: 'new' })}>
          New livery
        </a>
      </div>

      <Load state={state}>
        {({ projects }) => {
          if (projects.length === 0) {
            return (
              <Empty title="No liveries yet" boxed>
                Create a livery to set the car, template and brief.
              </Empty>
            );
          }
          const needle = search.trim().toLowerCase();
          const shown = projects.filter(
            (p) =>
              (carFilter === '' || p.car_id === carFilter) &&
              (needle === '' ||
                p.name.toLowerCase().includes(needle) ||
                carName(p.car_id).toLowerCase().includes(needle)),
          );
          if (shown.length === 0) {
            return (
              <Empty title="No matches" boxed>
                No livery matches the search and car filter.
              </Empty>
            );
          }
          return (
            <ul className="garage__grid">
              {shown.map((project) => (
                <li key={project.id}>
                  <a className="livery-card" href={routeHref({ name: 'studio', id: project.id })}>
                    <div className="livery-card__preview">
                      <span className="meta">NO RENDER YET</span>
                    </div>
                    <div className="livery-card__body">
                      <div className="livery-card__title">
                        <strong>{project.name}</strong>
                        <span className="meta">REV {project.revision_count}</span>
                      </div>
                      <span className="note">{carName(project.car_id)}</span>
                      <span className="tag">Draft</span>
                    </div>
                  </a>
                </li>
              ))}
            </ul>
          );
        }}
      </Load>
    </main>
  );
}
