import './Cars.css';

import { useState } from 'react';
import { Empty, Load, Row } from '../components/ui';
import { api, type CarDetail, type CarSummary } from '../lib/api';
import { formatInt, formatPercent } from '../lib/format';
import { routeHref } from '../lib/router';
import { useLoad } from '../lib/useLoad';

/** Calibration target: share of paintable area with a confirmed label. */
const COVERAGE_TARGET = 0.8;

export function Cars({ selectedId }: { selectedId: string | null }) {
  const { state } = useLoad(api.carsList, 'cars');
  return (
    <main className="screen cars">
      <Load state={state}>
        {(cars) => {
          const selected = cars.find((c) => c.id === selectedId) ?? cars[0];
          return (
            <div className="cars__columns">
              <CarList cars={cars} selectedId={selected?.id ?? null} />
              {selected ? (
                <CarPanels key={selected.id} id={selected.id} />
              ) : (
                <Empty title="No cars in the library" />
              )}
            </div>
          );
        }}
      </Load>
    </main>
  );
}

function CarList({ cars, selectedId }: { cars: CarSummary[]; selectedId: string | null }) {
  return (
    <section className="cars__list" aria-label="Cars">
      <div className="cars__list-title section-title">
        <span>Cars</span>
        <span>{cars.length}</span>
      </div>
      <nav className="cars__items">
        {cars.map((car) => (
          <a
            key={car.id}
            className="cars__item"
            href={routeHref({ name: 'cars', id: car.id })}
            aria-current={car.id === selectedId ? 'true' : undefined}
          >
            <span>{car.name}</span>
            <span className="mono">{formatPercent(car.confirmed_coverage_ratio)}</span>
          </a>
        ))}
      </nav>
      <div className="cars__add">
        <button type="button" className="btn btn--primary btn--block" disabled>
          Add car from template
        </button>
        <span className="note">
          Calibration for new cars is not available in this build. The library ships with{' '}
          {cars.length} calibrated cars.
        </span>
      </div>
    </section>
  );
}

function CarPanels({ id }: { id: string }) {
  const { state } = useLoad(() => api.carGet(id), `car:${id}`);
  return <Load state={state}>{(car) => <CarView car={car} />}</Load>;
}

function CarView({ car }: { car: CarDetail }) {
  const [panelId, setPanelId] = useState<string | null>(car.panels[0]?.id ?? null);
  const panel = car.panels.find((p) => p.id === panelId) ?? null;
  const onTarget = car.confirmed_coverage_ratio >= COVERAGE_TARGET;

  return (
    <>
      <section className="cars__map" aria-label="Island map">
        <div className="cars__map-head">
          <div className="subheader__title">
            <strong>{car.name}</strong>
            <span className="meta">
              {car.id.toUpperCase()} · PANELS <span className="data">{car.panel_count}</span> ·
              CONFIRMED{' '}
              <span className={onTarget ? 'data' : 'caution'}>
                {formatPercent(car.confirmed_coverage_ratio)}
              </span>{' '}
              · TARGET {formatPercent(COVERAGE_TARGET)}
            </span>
          </div>
          <div className="tabs" role="tablist">
            <button type="button" className="tab" role="tab" aria-selected="true">
              UV islands
            </button>
            <button type="button" className="tab" role="tab" aria-selected="false" disabled>
              Showroom
            </button>
            <button type="button" className="tab" role="tab" aria-selected="false" disabled>
              Patterns
            </button>
          </div>
        </div>
        <div className="cars__map-body">
          <div className="cars__map-frame">
            <Empty title="No island map">
              Import the iRacing template to view the island map
            </Empty>
          </div>
          <p className="note cars__map-note">
            Each island is painted a unique colour and id. Showroom captures confirm which panel is
            which, and which way is up.
          </p>
        </div>
      </section>

      <aside className="cars__atlas">
        <div className="cars__list-title section-title">
          <span>Panel atlas</span>
          <span>
            {car.confirmed_panel_count} CONFIRMED OF {car.panel_count}
          </span>
        </div>
        <div className="cars__table-wrap">
          <table className="cars__table">
            <thead>
              <tr>
                <th scope="col">Id</th>
                <th scope="col">Panel</th>
                <th scope="col">Up</th>
                <th scope="col">Mirror</th>
              </tr>
            </thead>
            <tbody>
              {car.panels.map((p) => (
                <tr
                  key={p.id}
                  aria-selected={p.id === panelId}
                  onClick={() => {
                    setPanelId(p.id);
                  }}
                >
                  <td>
                    <button
                      type="button"
                      className="cars__panel-btn"
                      onClick={() => {
                        setPanelId(p.id);
                      }}
                    >
                      <span
                        className="cars__swatch"
                        style={{ background: p.calibration_hex ?? 'transparent' }}
                      />
                      {p.id}
                    </button>
                  </td>
                  <td className={p.confidence === 'confirmed' ? '' : 'dim'}>{p.label ?? '—'}</td>
                  <td className="data">{p.up_angle_deg === null ? '—' : `${p.up_angle_deg}°`}</td>
                  <td className={p.mirror_of ? '' : 'dim'}>{p.mirror_of ?? 'none'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {panel ? (
          <div className="inset cars__selected">
            <h2 className="section-title">
              Selected · {panel.id} {panel.label ?? ''}
            </h2>
            <div className="rows rows--flush">
              <Row label="Forward" value={panel.forward ?? '—'} plain />
              <Row label="Area" value={`${formatInt(panel.area_px)} px`} plain />
              <Row
                label="Bounds"
                value={`${panel.bbox[0]}, ${panel.bbox[1]} → ${panel.bbox[2]}, ${panel.bbox[3]}`}
                plain
              />
              <Row
                label="Confidence"
                value={panel.confidence}
                tone={panel.confidence === 'confirmed' ? 'data' : 'caution'}
              />
              <Row
                label="Evidence"
                value={`${panel.evidence_count} ${panel.evidence_count === 1 ? 'capture' : 'captures'}`}
                tone="dim"
                plain
              />
            </div>
          </div>
        ) : null}
        <button type="button" className="btn" disabled>
          Re-run calibration
        </button>
      </aside>
    </>
  );
}
