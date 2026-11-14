import './Showroom.css';

import { Empty, Load } from '../components/ui';
import { routeHref } from '../lib/router';
import { projectCrumb, type ProjectView, useProject } from '../lib/useProject';

/** The nine showroom views, in capture-grid order. */
const SHOWROOM_VIEWS = [
  'FRONT_34_LEFT',
  'FRONT',
  'FRONT_34_RIGHT',
  'SIDE_LEFT',
  'TOP',
  'SIDE_RIGHT',
  'REAR_34_LEFT',
  'REAR',
  'REAR_34_RIGHT',
] as const;

const VERDICTS = [
  'ok',
  'wrong-panel',
  'rotated',
  'mirrored',
  'too-big',
  'too-small',
  'seam-mismatch',
  'colour-off',
  'finish-off',
];

export function Showroom({ id }: { id: string }) {
  const state = useProject(id);
  return (
    <main className="screen showroom">
      <Load state={state}>{(view) => <ShowroomView view={view} />}</Load>
    </main>
  );
}

function ShowroomView({ view }: { view: ProjectView }) {
  const { project } = view;
  return (
    <>
      <div className="subheader">
        <div className="subheader__title">
          <span className="meta">GARAGE / {projectCrumb(project)} / SHOWROOM</span>
          <strong>Showroom review</strong>
        </div>
        <div className="subheader__meta meta">
          <span>
            REV <span className="data">{project.revision_count}</span>
          </span>
          <span>
            VIEWS <span className="data">0/{SHOWROOM_VIEWS.length}</span>
          </span>
          <span>SPEC MIP NOT COMPILED</span>
        </div>
        <div className="subheader__actions">
          <button type="button" className="btn" disabled>
            Recapture
          </button>
          <a className="btn" href={routeHref({ name: 'studio', id: project.id })}>
            Back to studio
          </a>
        </div>
      </div>

      <div className="showroom__columns">
        <section className="showroom__grid" aria-label="Showroom views">
          {SHOWROOM_VIEWS.map((name) => (
            <figure key={name} className="showroom__view">
              <div className="showroom__frame">
                <span className="meta">NOT CAPTURED</span>
              </div>
              <figcaption className="meta">
                <span>{name}</span>
                <span>—</span>
              </figcaption>
            </figure>
          ))}
        </section>

        <aside className="showroom__review">
          <div className="showroom__review-title section-title">
            <span>Review items</span>
            <span>0</span>
          </div>
          <Empty title="No captures yet" boxed>
            Available after the first showroom capture. Capture needs a rendered revision
            installed in the iRacing Paint Shop.
          </Empty>
          <div className="showroom__verdicts">
            <span className="meta">Verdict per item</span>
            <div className="showroom__chips">
              {VERDICTS.map((verdict) => (
                <button key={verdict} type="button" className="btn btn--small mono" disabled>
                  {verdict}
                </button>
              ))}
            </div>
          </div>
          <div className="showroom__actions">
            <button type="button" className="btn btn--primary" disabled>
              Send feedback and render next revision
            </button>
            <button type="button" className="btn" disabled>
              Accept revision as final
            </button>
          </div>
        </aside>
      </div>
    </>
  );
}
