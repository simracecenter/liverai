import './Export.css';

import { Load, Row } from '../components/ui';
import { routeHref } from '../lib/router';
import { projectCrumb, type ProjectView, useProject } from '../lib/useProject';

export function ExportScreen({ id }: { id: string }) {
  const state = useProject(id);
  return (
    <main className="screen export">
      <Load state={state}>{(view) => <ExportView view={view} />}</Load>
    </main>
  );
}

function ExportView({ view }: { view: ProjectView }) {
  const { project } = view;
  const studio = routeHref({ name: 'studio', id: project.id });
  const custId = project.customer_id || '<id>';
  return (
    <section className="export__panel" aria-labelledby="export-title">
      <div className="export__head">
        <div className="export__title">
          <h1 id="export-title">Export for Trading Paints</h1>
          <span className="meta">
            {projectCrumb(project)} · REV <span className="data">{project.revision_count}</span> ·{' '}
            {project.car_id.toUpperCase()}
          </span>
        </div>
        <a className="btn btn--quiet export__close" href={studio} aria-label="Close">
          ×
        </a>
      </div>

      <div className="export__body">
        <div className="inset">
          <span className="section-title">Not available yet</span>
          <span className="note">
            Available after the first showroom capture is accepted. Export packages the paint and
            the spec map that iRacing compiled for that revision.
          </span>
        </div>

        <div className="export__group">
          <h2 className="section-title">Upload slots</h2>
          <div className="export__slots">
            <span>Paint file</span>
            <span className="mono dim">—</span>
            <span className="meta">NOT READY</span>
            <span>Spec map</span>
            <span className="mono dim">—</span>
            <span className="meta">NOT READY</span>
            <span>Decal layer</span>
            <span className="dim">Leave empty</span>
            <span className="meta">NONE</span>
          </div>
        </div>

        <div className="inset inset--caution">
          <span className="meta caution">CAUTION</span>
          <p className="export__caution">
            Upload the compiled <span className="mono">.mip</span> as the spec map, not the{' '}
            <span className="mono">.tga</span>. Do not put the spec map in the Decal layer slot:
            iRacing would stamp its raw channels on the car as green and brown paint.
          </p>
        </div>

        <div className="export__group">
          <h2 className="section-title">Checks</h2>
          <div className="rows">
            <Row label="Paint and spec rendered from the same revision" value="NOT RUN" tone="dim" />
            <Row
              label="Spec .mip compiled by iRacing after the last reload"
              value="NOT RUN"
              tone="dim"
            />
            <Row
              label={`No stray car_decal_${custId}.tga in the paint folder`}
              value="NOT RUN"
              tone="dim"
            />
            <Row label="Showroom review accepted" value="NOT RUN" tone="dim" />
          </div>
        </div>

        <div className="field">
          <label className="field__label" htmlFor="ex-folder">
            Export folder
          </label>
          <div className="export__folder">
            <input
              id="ex-folder"
              className="input input--mono"
              type="text"
              readOnly
              value={`${project.folder}${project.folder.includes("/") ? "/" : "\\"}export`}
            />
            <button type="button" className="btn" disabled>
              Change
            </button>
          </div>
        </div>
      </div>

      <div className="export__foot">
        <a className="btn" href={studio}>
          Cancel
        </a>
        <button type="button" className="btn btn--primary" disabled>
          Export and open folder
        </button>
      </div>
    </section>
  );
}
