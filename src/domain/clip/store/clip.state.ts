import type { Filter } from '@/domain/clip/apis/clip.dto';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';

/** The panel's search; kept between openings (the window is only hidden), never persisted. */
export interface ClipState {
  query: string;
  filter: Filter;
  /** The app tag in the search box, or null. */
  app: App | null;
  setQuery: (query: string) => void;
  setFilter: (filter: Filter) => void;
  setApp: (app: App | null) => void;
  /** Clears the search, app tag and filter. */
  resetSearch: () => void;
}
