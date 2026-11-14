import './Studio.css';

import { Empty, Load, Row } from '../components/ui';
import { fileName, formatCanvas } from '../lib/format';
import { routeHref } from '../lib/router';
import { projectCrumb, type ProjectView, useProject } from '../lib/useProject';

const LINT_CHECKS = [
  'Format',
  'Number blocks overlap',
  'Car mandatory overlap',
  'Outside paintable mask',
  'Hardware spec preserved',
  'Diffuse / spec in sync',
];

const OVERLAYS = ['Wire', 'Number blocks', 'Mandatory', 'Panel ids'];

export function Studio({ id }: { id: string }) {
  const state = useProject(id);
  return (
    <main className="screen studio">
      <Load state={state}>{(view) => <StudioView view={view} />}</Load>
    </main>
  );
}

function StudioView({ view }: { view: ProjectView }) {
  const { project, car } = view;
  const rev = project.revision_count;
  return (
    <>
      <div className="subheader">
        <div className="subheader__title">
          <span className="meta">GARAGE / {projectCrumb(project)}</span>
          <strong>
            {car?.name ?? project.car_id} · {project.name}
          </strong>
        </div>
        <div className="subheader__meta meta">
          <span>
            REV <span className="data">{rev}</span>
          </span>
          <span>
            CUST ID{' '}
            {project.customer_id ? (
              <span className="data">{project.customer_id}</span>
            ) : (
              <span className="caution">NOT SET</span>
            )}
          </span>
          <span>
            CANVAS <span className="data">{car ? formatCanvas(car.canvas) : '—'}</span>
          </span>
        </div>
        <div className="subheader__actions">
          <button type="button" className="btn" disabled>
            Revisions
          </button>
          <a className="btn" href={routeHref({ name: 'export', id: project.id })}>
            Export
          </a>
        </div>
      </div>

      <div className="studio__columns">
        <section className="studio__log" aria-label="Run log">
          <h2 className="section-title studio__pad">Run log · REV {rev}</h2>
          <div className="studio__log-empty">
            <Empty title="No runs yet">The run log fills in when a design run starts.</Empty>
          </div>

          <div className="inset studio__brief">
            <h2 className="section-title">Brief</h2>
            {project.brief.trim() ? (
              <p className="studio__brief-text">{project.brief}</p>
            ) : (
              <p className="dim">No brief written.</p>
            )}
            <button type="button" className="btn btn--primary btn--block" disabled>
              Start design run
            </button>
            <span className="note">Design runs are not available in this build.</span>
          </div>

          <div className="studio__prompt">
            <label className="meta" htmlFor="studio-prompt">
              Request a change
            </label>
            <div className="studio__prompt-row">
              <input
                id="studio-prompt"
                className="input"
                type="text"
                placeholder="Available after the first run"
                disabled
              />
              <button type="button" className="btn" disabled>
                Send
              </button>
            </div>
          </div>
        </section>

        <section className="studio__viewer" aria-label="Viewer">
          <div className="tabs studio__tabs" role="tablist">
            <button type="button" className="tab" role="tab" aria-selected="true">
              UV preview
            </button>
            <button type="button" className="tab" role="tab" aria-selected="false" disabled>
              Spec map
            </button>
            <a className="tab" href={routeHref({ name: 'showroom', id: project.id })}>
              Showroom
            </a>
            <button type="button" className="tab" role="tab" aria-selected="false" disabled>
              Diff
            </button>
            <div className="studio__overlays">
              {OVERLAYS.map((overlay) => (
                <label key={overlay}>
                  <input type="checkbox" disabled />
                  {overlay}
                </label>
              ))}
            </div>
          </div>
          <div className="studio__canvas">
            <div className="studio__canvas-frame">
              <Empty title="No render yet">
                The UV preview appears after the first render.
              </Empty>
            </div>
          </div>
          <div className="studio__readout meta">
            <span>ZOOM —</span>
            <span>CURSOR —</span>
            <span>PANEL —</span>
            <span>UP —</span>
          </div>
        </section>

        <aside className="studio__inspector">
          <div className="studio__group">
            <div className="studio__group-title section-title">
              <span>Lint</span>
              <span>Not run</span>
            </div>
            <div className="rows rows--flush">
              {LINT_CHECKS.map((check) => (
                <Row key={check} label={check} value="—" tone="dim" />
              ))}
            </div>
          </div>

          <div className="studio__group">
            <h2 className="section-title">Palette and finish</h2>
            <span className="note">No palette yet. Colours and finishes are set by the design.</span>
            <span className="meta">SPEC MAP R METALLIC · G ROUGHNESS · B CLEAR COAT</span>
          </div>

          <div className="studio__group">
            <div className="studio__group-title section-title">
              <span>Decals</span>
              <span>0</span>
            </div>
            <span className="note">No decals placed.</span>
          </div>

          <div className="studio__group">
            <h2 className="section-title">Project</h2>
            <div className="rows rows--flush">
              <Row
                label="Template"
                value={project.template_path ? fileName(project.template_path) : 'NOT SELECTED'}
                tone={project.template_path ? 'text' : 'caution'}
                plain={project.template_path !== null}
              />
              <Row
                label="Panel atlas"
                value={car ? `${car.panel_count} PANELS` : 'CAR NOT IN LIBRARY'}
                tone={car ? 'data' : 'caution'}
              />
              <Row label="Folder" value={project.folder} tone="dim" plain />
            </div>
          </div>
        </aside>
      </div>
    </>
  );
}
