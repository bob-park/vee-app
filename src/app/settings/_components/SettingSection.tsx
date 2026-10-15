/** Bordered card that groups setting rows. */
export default function SettingSection({ children }: Readonly<{ children: React.ReactNode }>) {
  return <section className="border-base-300 bg-card mb-4 rounded-xl border px-4">{children}</section>;
}
