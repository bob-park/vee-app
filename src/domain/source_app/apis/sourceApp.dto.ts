export interface App {
  id: number;
  name: string;
  icon: string | null;
  /** How many clips came from this app. */
  count: number;
}
