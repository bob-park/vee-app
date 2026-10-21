/** Glass card that groups setting rows; springs in, `index` staggering it after the ones above. */
export default function SettingSection({
  children,
  index = 0,
}: Readonly<{ children: React.ReactNode; index?: number }>) {
  return (
    <section
      className="glass shadow-glass animate-spring-in mb-4 rounded-xl px-4"
      style={{ animationDelay: `${index * 60}ms` }}
    >
      {children}
    </section>
  );
}
