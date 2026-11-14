import './NewLivery.css';

import { type SyntheticEvent, useState } from 'react';
import { Alert, Load, Row } from '../components/ui';
import {
  api,
  type CarNumberMode,
  type CarSummary,
  type ClaudeStatus,
  errorMessage,
  type PaintType,
  type Settings,
} from '../lib/api';
import { connectionLabel } from '../lib/claude';
import { fileName, formatPercent, isCustomerId } from '../lib/format';
import { navigate, routeHref } from '../lib/router';
import { type Loadable, useLoad } from '../lib/useLoad';

interface FormData {
  cars: CarSummary[];
  settings: Settings;
}

async function loadForm(): Promise<FormData> {
  const [cars, settings] = await Promise.all([api.carsList(), api.settingsGet()]);
  return { cars, settings };
}

export function NewLivery({ claude }: { claude: Loadable<ClaudeStatus> }) {
  const { state } = useLoad(loadForm, 'new-livery');
  return (
    <main className="screen new-livery">
      <Load state={state}>{(data) => <NewLiveryForm data={data} claude={claude} />}</Load>
    </main>
  );
}

function NewLiveryForm({ data, claude }: { data: FormData; claude: Loadable<ClaudeStatus> }) {
  const { cars, settings } = data;
  const [name, setName] = useState('');
  const [carId, setCarId] = useState(cars[0]?.id ?? '');
  const [templatePath, setTemplatePath] = useState<string | null>(null);
  const [paintType, setPaintType] = useState<PaintType>('driver');
  const [customerId, setCustomerId] = useState(settings.customer_id);
  const [carNumber, setCarNumber] = useState<CarNumberMode>('sim');
  const [brief, setBrief] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const car = cars.find((c) => c.id === carId);
  const customerIdValid = isCustomerId(customerId);
  const canCreate = name.trim() !== '' && car !== undefined && customerIdValid && !busy;
  const connection = connectionLabel(claude);

  const browse = () => {
    setError(null);
    api.pickTemplate().then(
      (picked) => {
        if (picked) setTemplatePath(picked);
      },
      (err: unknown) => {
        setError(errorMessage(err));
      },
    );
  };

  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    if (!canCreate) return;
    setBusy(true);
    setError(null);
    api
      .projectCreate({
        name: name.trim(),
        car_id: carId,
        template_path: templatePath,
        customer_id: customerId,
        brief,
        paint_type: paintType,
        car_number: carNumber,
      })
      .then(
        (project) => {
          navigate({ name: 'studio', id: project.id });
        },
        (err: unknown) => {
          setError(errorMessage(err));
          setBusy(false);
        },
      );
  };

  return (
    <form className="new-livery__layout" onSubmit={submit}>
      <div className="new-livery__main">
        <div className="new-livery__heading">
          <span className="meta">GARAGE / NEW LIVERY</span>
          <h1 className="page-title">New livery</h1>
        </div>

        <section className="card">
          <h2 className="section-title">Car and template</h2>
          <div className="field">
            <label className="field__label" htmlFor="nl-name">
              Livery name
            </label>
            <input
              id="nl-name"
              className="input"
              type="text"
              maxLength={80}
              required
              value={name}
              onChange={(e) => {
                setName(e.target.value);
              }}
            />
          </div>
          <div className="form-row">
            <div className="field new-livery__half">
              <label className="field__label" htmlFor="nl-car">
                Car
              </label>
              <select
                id="nl-car"
                className="input"
                value={carId}
                onChange={(e) => {
                  setCarId(e.target.value);
                }}
              >
                {cars.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
              {car ? (
                <span className="meta">
                  ATLAS <span className="data">CALIBRATED</span> · {car.panel_count} PANELS ·{' '}
                  <span className="data">{formatPercent(car.confirmed_coverage_ratio)}</span>
                </span>
              ) : null}
            </div>
            <div className="field new-livery__half">
              <span className="field__label" id="nl-template-label">
                iRacing template
              </span>
              <div className="file-box" aria-labelledby="nl-template-label" role="group">
                <span className={`file-box__name ${templatePath ? 'mono' : 'dim'}`}>
                  {templatePath ? fileName(templatePath) : 'No template selected'}
                </span>
                <button type="button" className="btn btn--small" onClick={browse}>
                  {templatePath ? 'Replace' : 'Browse'}
                </button>
              </div>
              <span className="meta">
                {templatePath ? 'SELECTED · NOT INSPECTED' : '.PSD · OPTIONAL FOR NOW'}
              </span>
            </div>
          </div>
          <span className="note">
            Templates stay on this PC. Download them from iRacing; Livery AI does not redistribute
            them.
          </span>
        </section>

        <section className="card">
          <h2 className="section-title">Paint target</h2>
          <div className="form-row">
            <div className="field new-livery__third">
              <label className="field__label" htmlFor="nl-type">
                File type
              </label>
              <select
                id="nl-type"
                className="input"
                value={paintType}
                onChange={(e) => {
                  setPaintType(e.target.value === 'team' ? 'team' : 'driver');
                }}
              >
                <option value="driver">Driver paint (car_&lt;id&gt;.tga)</option>
                <option value="team">Team paint (car_team_&lt;id&gt;.tga)</option>
              </select>
            </div>
            <div className="field new-livery__third">
              <label className="field__label" htmlFor="nl-cust">
                Customer ID
              </label>
              <input
                id="nl-cust"
                className="input input--data"
                type="text"
                inputMode="numeric"
                autoComplete="off"
                aria-invalid={!customerIdValid}
                aria-describedby="nl-cust-hint"
                value={customerId}
                onChange={(e) => {
                  setCustomerId(e.target.value.trim());
                }}
              />
              <span id="nl-cust-hint" className={`meta ${customerIdValid ? '' : 'caution'}`}>
                {!customerIdValid
                  ? 'DIGITS ONLY'
                  : customerId === ''
                    ? 'NOT SET'
                    : customerId === settings.customer_id
                      ? 'FROM SETTINGS'
                      : 'THIS LIVERY ONLY'}
              </span>
            </div>
            <div className="field new-livery__third">
              <label className="field__label" htmlFor="nl-num">
                Car number
              </label>
              <select
                id="nl-num"
                className="input"
                value={carNumber}
                onChange={(e) => {
                  setCarNumber(e.target.value === 'custom' ? 'custom' : 'sim');
                }}
              >
                <option value="sim">Stamped by the sim</option>
                <option value="custom">Custom number paint</option>
              </select>
            </div>
          </div>
        </section>

        <section className="card">
          <h2 className="section-title">Brief</h2>
          <div className="field">
            <label className="field__label" htmlFor="nl-brief">
              Describe the livery
            </label>
            <textarea
              id="nl-brief"
              className="input"
              rows={5}
              placeholder="Base colour, pattern, finishes, logos and where they go."
              value={brief}
              onChange={(e) => {
                setBrief(e.target.value);
              }}
            />
          </div>
          <div className="form-row">
            <div className="field new-livery__half">
              <label className="field__label" htmlFor="nl-kit">
                Brand kit
              </label>
              <select id="nl-kit" className="input" disabled>
                <option>No brand kit</option>
              </select>
              <span className="note">Brand kits are not available in this build.</span>
            </div>
            <div className="field new-livery__half">
              <span className="field__label">Extra logos for this livery</span>
              <div className="file-box">
                <span className="file-box__name dim">PNG or SVG files you own</span>
                <button type="button" className="btn btn--small" disabled>
                  Browse
                </button>
              </div>
              <span className="note">Logo import is not available in this build.</span>
            </div>
          </div>
        </section>
      </div>

      <aside className="new-livery__aside">
        <section className="card">
          <h2 className="section-title">Run plan</h2>
          <ol className="new-livery__plan">
            <li>Inspect the template layers and keep-out zones.</li>
            <li>Pick a seam-safe base pattern and place decals on the panel atlas.</li>
            <li>Render the paint and spec map, then lint.</li>
            <li>Ask before installing to the paint folder.</li>
            <li>Reload the Paint Shop and capture 9 showroom views.</li>
            <li>Review the views against the brief and report.</li>
          </ol>
        </section>
        <section className="card new-livery__preflight">
          <h2 className="section-title">Pre-flight</h2>
          <div className="rows rows--flush">
            <Row label="Claude" value={connection.text} tone={connection.tone} />
            <Row
              label="Template"
              value={templatePath ? 'SELECTED' : 'NOT SELECTED'}
              tone={templatePath ? 'data' : 'caution'}
            />
            <Row
              label="Panel atlas"
              value={car ? 'CALIBRATED' : 'NO CAR'}
              tone={car ? 'data' : 'caution'}
            />
            <Row label="Paint Shop" value="NOT CHECKED" tone="dim" />
          </div>
          <span className="note">
            Design runs are not available in this build. Creating the livery saves the car,
            template and brief, then opens the Studio.
          </span>
        </section>
        {error ? <Alert>{error}</Alert> : null}
        <button type="submit" className="btn btn--primary new-livery__submit" disabled={!canCreate}>
          {busy ? 'Creating' : 'Create livery'}
        </button>
        <a className="btn btn--quiet" href={routeHref({ name: 'garage' })}>
          Cancel
        </a>
      </aside>
    </form>
  );
}
