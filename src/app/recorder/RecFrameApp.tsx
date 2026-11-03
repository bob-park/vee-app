/** The dashed border around the area being recorded. Rust places it just outside the area and lets clicks through. */
export default function RecFrameApp() {
  return <div className="border-rec fixed inset-0 rounded-[3px] border-2 border-dashed" aria-hidden />;
}
