// Small shared building blocks.
import type { ReactNode } from 'react';
import type { Loadable } from '../lib/useLoad';

export function Empty({
  title,
  children,
  boxed = false,
}: {
  title: string;
  children?: ReactNode;
  boxed?: boolean;
}) {
  return (
    <div className={boxed ? 'empty empty--boxed' : 'empty'}>
      <span className="empty__title">{title}</span>
      {children ? <span>{children}</span> : null}
    </div>
  );
}

export function Alert({ children }: { children: ReactNode }) {
  return (
    <div className="alert" role="alert">
      <strong>ERROR</strong>
      {children}
    </div>
  );
}

/** Render loading and error states for core data, then the ready view. */
export function Load<T>({
  state,
  children,
}: {
  state: Loadable<T>;
  children: (data: T) => ReactNode;
}) {
  if (state.status === 'loading') return <Empty title="Loading" />;
  if (state.status === 'error') {
    return (
      <div className="empty">
        <Alert>{state.error}</Alert>
      </div>
    );
  }
  return <>{children(state.data)}</>;
}

/** One label / value line. `tone` colours the value. */
export function Row({
  label,
  value,
  tone = 'data',
  plain = false,
}: {
  label: ReactNode;
  value: ReactNode;
  tone?: 'data' | 'dim' | 'caution' | 'critical' | 'text';
  plain?: boolean;
}) {
  const toneClass = tone === 'text' ? '' : tone;
  return (
    <div className="rows__row">
      <span>{label}</span>
      <span className={`rows__value ${plain ? 'rows__value--plain' : ''} ${toneClass}`}>
        {value}
      </span>
    </div>
  );
}
