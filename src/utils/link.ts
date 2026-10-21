function parse(line: string): URL | null {
  try {
    return new URL(line);
  } catch {
    if (line.includes('://')) return null;
    try {
      return new URL(`https://${line}`);
    } catch {
      return null;
    }
  }
}

/** Splits a copied link into the domain shown in bold and the full first line under it. */
export function linkParts(text: string): { host: string; rest: string } {
  const rest = text.split('\n')[0].trim();
  const url = parse(rest);
  // `mailto:` and the like have no host, so the scheme stands in for it.
  const host = url ? url.hostname.replace(/^www\./, '') || url.protocol.slice(0, -1) : rest.split('/')[0];
  return { host: host || rest, rest };
}
