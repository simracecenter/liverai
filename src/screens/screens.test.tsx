import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import { api } from '../lib/api';
import { appInfo, carDetail, cars, claudeReady, project, settings } from '../test/fixtures';

vi.mock('../lib/api', async (importOriginal) => {
  const original = await importOriginal<typeof import('../lib/api')>();
  return {
    ...original,
    api: {
      appInfo: vi.fn(),
      settingsGet: vi.fn(),
      settingsSet: vi.fn(),
      carsList: vi.fn(),
      carGet: vi.fn(),
      projectsList: vi.fn(),
      projectCreate: vi.fn(),
      projectGet: vi.fn(),
      claudeStatus: vi.fn(),
      claudeOpenSignin: vi.fn(),
      claudeTest: vi.fn(),
      pickTemplate: vi.fn(),
      pickFolder: vi.fn(),
    },
  };
});

const mocked = vi.mocked(api);

function open(hash: string) {
  window.location.hash = hash;
  return render(<App />);
}

beforeEach(() => {
  vi.resetAllMocks();
  mocked.appInfo.mockResolvedValue(appInfo);
  mocked.settingsGet.mockResolvedValue(settings);
  mocked.settingsSet.mockImplementation((s) => Promise.resolve(s));
  mocked.carsList.mockResolvedValue(cars);
  mocked.carGet.mockResolvedValue(carDetail);
  mocked.projectsList.mockResolvedValue([]);
  mocked.projectGet.mockResolvedValue(project);
  mocked.projectCreate.mockResolvedValue(project);
  mocked.claudeStatus.mockResolvedValue(claudeReady);
});

describe('navigation', () => {
  it('has Garage, Cars and Settings and no Brand kits', async () => {
    open('');
    const nav = screen.getByRole('navigation', { name: 'Main' });
    expect(within(nav).getAllByRole('link').map((a) => a.textContent)).toEqual([
      'Garage',
      'Cars',
      'Settings',
    ]);
    expect(within(nav).getByRole('link', { name: 'Garage' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(await screen.findByText(/Claude · SIGNED IN/)).toBeInTheDocument();
  });
});

describe('Garage', () => {
  it('shows an empty state and one primary action', async () => {
    open('#/garage');
    expect(await screen.findByText('No liveries yet')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'New livery' })).toHaveAttribute('href', '#/new');
  });

  it('lists real projects and filters them', async () => {
    mocked.projectsList.mockResolvedValue([
      project,
      { ...project, id: 'gt3', name: 'Red GT3', car_id: 'ferrari296gt3', revision_count: 2 },
    ]);
    open('#/garage');
    const card = await screen.findByRole('link', { name: /Works livery/ });
    expect(card).toHaveAttribute('href', '#/studio/works-livery');
    expect(within(card).getByText('Porsche 911 GT3 R (992)')).toBeInTheDocument();
    expect(within(card).getByText('REV 0')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /Red GT3/ })).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText('Search liveries'), 'ferrari');
    expect(screen.queryByRole('link', { name: /Works livery/ })).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: /Red GT3/ })).toBeInTheDocument();
  });

  it('shows the core error', async () => {
    mocked.projectsList.mockRejectedValue({ code: 'io', message: 'Read projects folder: denied' });
    open('#/garage');
    expect(await screen.findByRole('alert')).toHaveTextContent('Read projects folder: denied');
  });
});

describe('New livery', () => {
  it('creates the project through the core and opens the Studio', async () => {
    mocked.pickTemplate.mockResolvedValue('C:\\templates\\Porsche 992r GT3.psd');
    open('#/new');
    const create = await screen.findByRole('button', { name: 'Create livery' });
    expect(create).toBeDisabled();
    expect(screen.getByLabelText('Customer ID')).toHaveValue('123456');
    expect(screen.getByText('FROM SETTINGS')).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText('Livery name'), 'Works livery');
    await userEvent.selectOptions(screen.getByLabelText('Car'), 'ferrari296gt3');
    const template = screen.getByRole('group', { name: 'iRacing template' });
    await userEvent.click(within(template).getByRole('button', { name: 'Browse' }));
    expect(await screen.findByText('Porsche 992r GT3.psd')).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText('Describe the livery'), 'Apex Orange body.');
    await userEvent.click(create);

    expect(mocked.projectCreate).toHaveBeenCalledWith({
      name: 'Works livery',
      car_id: 'ferrari296gt3',
      template_path: 'C:\\templates\\Porsche 992r GT3.psd',
      customer_id: '123456',
      brief: 'Apex Orange body.',
      paint_type: 'driver',
      car_number: 'sim',
    });
    await waitFor(() => {
      expect(window.location.hash).toBe('#/studio/works-livery');
    });
  });

  it('blocks a customer id that is not digits', async () => {
    open('#/new');
    await userEvent.type(await screen.findByLabelText('Livery name'), 'X');
    await userEvent.type(screen.getByLabelText('Customer ID'), 'abc');
    expect(screen.getByText('DIGITS ONLY')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create livery' })).toBeDisabled();
  });
});

describe('Studio', () => {
  it('shows project data with honest empty states', async () => {
    open('#/studio/works-livery');
    expect(await screen.findByText('Porsche 911 GT3 R (992) · Works livery')).toBeInTheDocument();
    expect(mocked.projectGet).toHaveBeenCalledWith('works-livery');
    expect(screen.getByText('GARAGE / WORKS-LIVERY')).toBeInTheDocument();
    expect(screen.getByText('Apex Orange body.')).toBeInTheDocument();
    expect(screen.getByText('No runs yet')).toBeInTheDocument();
    expect(screen.getByText('No render yet')).toBeInTheDocument();
    expect(screen.getByText('Not run')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start design run' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    expect(screen.getByRole('link', { name: 'Export' })).toHaveAttribute(
      'href',
      '#/studio/works-livery/export',
    );
    expect(screen.getByRole('link', { name: 'Showroom' })).toHaveAttribute(
      'href',
      '#/studio/works-livery/showroom',
    );
  });

  it('reports a missing project', async () => {
    mocked.projectGet.mockRejectedValue({ code: 'not_found', message: 'Project was not found.' });
    open('#/studio/nope');
    expect(await screen.findByRole('alert')).toHaveTextContent('Project was not found.');
  });
});

describe('Showroom and Export', () => {
  it('Showroom shows nine uncaptured views and disabled actions', async () => {
    open('#/studio/works-livery/showroom');
    expect(await screen.findByText('Showroom review')).toBeInTheDocument();
    expect(screen.getAllByText('NOT CAPTURED')).toHaveLength(9);
    expect(screen.getByText(/Available after the first showroom capture/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Send feedback/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Recapture' })).toBeDisabled();
  });

  it('Export is disabled until a capture is accepted', async () => {
    open('#/studio/works-livery/export');
    expect(await screen.findByText('Export for Trading Paints')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Export and open folder' })).toBeDisabled();
    expect(screen.getAllByText('NOT RUN')).toHaveLength(4);
    expect(screen.getByLabelText('Export folder')).toHaveValue(
      'C:\\Users\\driver\\Documents\\Livery AI\\works-livery\\export',
    );
  });
});

describe('Cars', () => {
  it('lists cars with coverage and the panel table for the selected car', async () => {
    open('#/cars');
    expect(await screen.findByRole('link', { name: /Ferrari 296 GT3/ })).toHaveTextContent('80.5%');
    expect(await screen.findByText('body_side_right')).toBeInTheDocument();
    expect(mocked.carGet).toHaveBeenCalledWith('porsche992rgt3');
    expect(
      screen.getByText('Import the iRacing template to view the island map'),
    ).toBeInTheDocument();
    expect(screen.getByText('363,974 px')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'p002' }));
    expect(screen.getByText('100 px')).toBeInTheDocument();
    expect(screen.getByText('guess')).toHaveClass('caution');
  });
});

describe('Settings', () => {
  it('offers subscription sign-in only and says Powered by Claude', async () => {
    open('#/settings');
    expect(await screen.findByText('Claude subscription')).toBeInTheDocument();
    expect(screen.getByText('DETECTED · 2.1.325')).toBeInTheDocument();
    expect(screen.getByText('ACTIVE · SUBSCRIPTION')).toBeInTheDocument();
    expect(screen.getByText('NOT RUN')).toBeInTheDocument();
    expect(screen.getAllByText(/Powered by Claude/i).length).toBeGreaterThan(0);
    expect(screen.queryByText(/API key/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/API key/i)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Model')).toHaveValue('claude-opus-5-5');
    expect(screen.getByLabelText('Effort')).toHaveValue('high');
  });

  it('saves the model id and permission toggles', async () => {
    open('#/settings');
    const save = await screen.findByRole('button', { name: 'Save settings' });
    expect(save).toBeDisabled();
    await userEvent.selectOptions(screen.getByLabelText('Model'), 'claude-fable-5-2');
    await userEvent.click(screen.getByLabelText('Ask before writing to the paint folder'));
    expect(screen.getByText('UNSAVED CHANGES')).toBeInTheDocument();
    await userEvent.click(save);
    expect(mocked.settingsSet).toHaveBeenCalledWith({
      ...settings,
      model: 'claude-fable-5-2',
      ask_before_install: false,
    });
    expect(await screen.findByText('SAVED')).toBeInTheDocument();
  });

  it('runs the connection test and reports the result', async () => {
    mocked.claudeTest.mockResolvedValue({ ok: true, elapsed_ms: 1840, reason: null, exit_code: 0 });
    open('#/settings');
    await userEvent.click(await screen.findByRole('button', { name: 'Test connection' }));
    expect(await screen.findByText('PASS · 1.8 S')).toBeInTheDocument();
  });

  it('flags a sign-in that is not a subscription', async () => {
    mocked.claudeStatus.mockResolvedValue({
      ...claudeReady,
      sign_in: { state: 'signed_out', method: null },
    });
    open('#/settings');
    expect(await screen.findByText('NOT SIGNED IN')).toHaveClass('caution');
  });
});
