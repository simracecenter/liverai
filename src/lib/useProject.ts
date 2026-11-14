import { api, type CarDetail, type ProjectDetail } from './api';
import { type Loadable, useLoad } from './useLoad';

export interface ProjectView {
  project: ProjectDetail;
  /** Null when the project's car is no longer in the library. */
  car: CarDetail | null;
}

async function loadProject(id: string): Promise<ProjectView> {
  const project = await api.projectGet(id);
  const car = await api.carGet(project.car_id).catch(() => null);
  return { project, car };
}

/** A project and its car, for the Studio, Showroom and Export screens. */
export function useProject(id: string): Loadable<ProjectView> {
  return useLoad(() => loadProject(id), `project:${id}`).state;
}

/** Breadcrumb id: the project folder name in capitals. */
export function projectCrumb(project: ProjectDetail): string {
  return project.id.toUpperCase();
}
