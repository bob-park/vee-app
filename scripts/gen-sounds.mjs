// Generates the built-in copy sounds into src-tauri/sounds/*.wav.
// Run: node scripts/gen-sounds.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RATE = 44100;
const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "sounds");

/** Exponential ramp from a to b over t ∈ [0, 1]. */
const ramp = (a, b, t) => a * Math.pow(b / a, t);

/** A pitch-swept tone with a 5ms attack and an exponential decay, like the mockup's WebAudio version. */
function tone(buf, { f0, f1, start = 0, dur, vol = 0.35, wave = "sine" }) {
  const begin = Math.round(start * RATE);
  const n = Math.round(dur * RATE);
  const attack = Math.round(0.005 * RATE);
  let phase = 0;
  for (let i = 0; i < n && begin + i < buf.length; i++) {
    const t = i / n;
    phase += (2 * Math.PI * ramp(f0, f1, t)) / RATE;
    const env = i < attack ? ramp(0.0001, vol, i / attack) : ramp(vol, 0.0001, (i - attack) / (n - attack));
    const x = phase / (2 * Math.PI);
    const s = wave === "triangle" ? 4 * Math.abs(x - Math.floor(x + 0.5)) - 1 : Math.sin(phase);
    buf[begin + i] += s * env;
  }
}

/** A short decaying noise burst; seeded so the output is reproducible. */
function noise(buf, { dur, vol }) {
  let seed = 1;
  const n = Math.round(dur * RATE);
  for (let i = 0; i < n; i++) {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    buf[i] += ((seed / 2 ** 32) * 2 - 1) * Math.pow(1 - i / n, 4) * vol;
  }
}

const SOUNDS = {
  pop: { len: 0.11, draw: (b) => tone(b, { f0: 880, f1: 330, dur: 0.09 }) },
  click: { len: 0.045, draw: (b) => noise(b, { dur: 0.025, vol: 0.6 }) },
  chime: {
    len: 0.36,
    draw: (b) => {
      tone(b, { f0: 1046.5, f1: 1046.5, dur: 0.18, vol: 0.25 });
      tone(b, { f0: 1568, f1: 1568, start: 0.09, dur: 0.25, vol: 0.22 });
    },
  },
  bubble: { len: 0.14, draw: (b) => tone(b, { f0: 400, f1: 1200, dur: 0.12, vol: 0.3 }) },
  tap: { len: 0.1, draw: (b) => tone(b, { f0: 220, f1: 160, dur: 0.08, vol: 0.45, wave: "triangle" }) },
};

/** 16-bit mono PCM WAV. */
function wav(samples) {
  const data = Buffer.alloc(samples.length * 2);
  samples.forEach((s, i) => data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, s)) * 32767), i * 2));
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(RATE, 24);
  header.writeUInt32LE(RATE * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write("data", 36);
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

mkdirSync(OUT, { recursive: true });
for (const [name, { len, draw }] of Object.entries(SOUNDS)) {
  const buf = new Float64Array(Math.round(len * RATE));
  draw(buf);
  writeFileSync(join(OUT, `${name}.wav`), wav(buf));
  console.log(`${name}.wav`);
}
