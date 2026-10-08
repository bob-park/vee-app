import { cursorCentredFrame } from "./motion.ts";

/**
 * Draws a card into a PNG for the OS drag image, so the card that floats inside the panel
 * keeps its look once the drag leaves the window. Covers what file and image cards are made
 * of: boxes (background, border, radius), images, SVG icons and single-line text.
 *
 * Styles and positions are read immediately (before the card turns into a slot); only SVG
 * icons load asynchronously. The image is drawn at 2x and at the floating card's scale, padded
 * so the grabbed point is its centre, because the native drag centres the image on the cursor.
 */
// async so that a failure while reading the card becomes a rejection, never a broken drag;
// everything before the first await still runs immediately.
export async function cardImage(card: HTMLElement, cursorX: number, cursorY: number, zoom: number): Promise<string> {
  const r = card.getBoundingClientRect();
  const w = r.width * zoom;
  const h = r.height * zoom;
  // The floating card is scaled around its centre, so that is where the cursor offset is measured from.
  const ox = cursorX - (r.left + r.width / 2) + w / 2;
  const oy = cursorY - (r.top + r.height / 2) + h / 2;
  const frame = cursorCentredFrame(w, h, ox, oy);

  const ops: ((ctx: CanvasRenderingContext2D) => void)[] = [];
  const loads: Promise<unknown>[] = [];
  paint(card, ops, loads);

  await Promise.all(loads);
  const px = 2;
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(frame.width * px);
  canvas.height = Math.ceil(frame.height * px);
  const ctx = canvas.getContext("2d")!;
  ctx.scale(px, px);
  ctx.translate(frame.x, frame.y);
  ctx.scale(zoom, zoom);
  ctx.translate(-r.left, -r.top);
  for (const op of ops) op(ctx);
  return canvas.toDataURL("image/png").slice("data:image/png;base64,".length);
}

function paint(el: Element, ops: ((ctx: CanvasRenderingContext2D) => void)[], loads: Promise<unknown>[]): void {
  // getComputedStyle is live: copy every value now, before the card turns into a dashed slot.
  const cs = getComputedStyle(el);
  if (cs.display === "none" || cs.visibility === "hidden" || cs.opacity === "0") return;
  const rect = el.getBoundingClientRect();
  const radius = parseFloat(cs.borderTopLeftRadius) || 0;
  const alpha = parseFloat(cs.opacity);
  const background = cs.backgroundColor;
  const border = parseFloat(cs.borderTopWidth) || 0;
  const borderStyle = cs.borderTopStyle;
  const borderColor = cs.borderTopColor;

  if (el instanceof SVGSVGElement) {
    const img = new Image();
    img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(inlineSvg(el))}`;
    loads.push(img.decode().catch(() => {}));
    ops.push((ctx) => img.complete && img.naturalWidth > 0 && ctx.drawImage(img, rect.left, rect.top, rect.width, rect.height));
    return;
  }

  const clips = cs.overflow !== "visible";
  ops.push((ctx) => {
    ctx.save();
    ctx.globalAlpha *= alpha;
    if (isPainted(background)) {
      ctx.fillStyle = background;
      ctx.beginPath();
      ctx.roundRect(rect.left, rect.top, rect.width, rect.height, radius);
      ctx.fill();
    }
    if (clips) {
      ctx.beginPath();
      ctx.roundRect(rect.left, rect.top, rect.width, rect.height, radius);
      ctx.clip();
    }
  });

  if (el instanceof HTMLImageElement && el.complete && el.naturalWidth > 0) {
    const faded = cs.webkitMaskImage.includes("gradient") || cs.maskImage.includes("gradient");
    const fit = cs.objectFit;
    ops.push((ctx) => drawImage(ctx, el, rect, fit, radius, faded));
  }

  for (const node of el.childNodes) {
    if (node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) text(node as Text, cs, ops);
    else if (node instanceof Element) paint(node, ops, loads);
  }

  ops.push((ctx) => ctx.restore());
  // Borders go on top of the content, as the browser draws them.
  if (border > 0 && borderStyle !== "none" && isPainted(borderColor)) {
    ops.push((ctx) => {
      ctx.save();
      ctx.globalAlpha *= alpha;
      ctx.strokeStyle = borderColor;
      ctx.lineWidth = border;
      const half = border / 2;
      ctx.beginPath();
      ctx.roundRect(rect.left + half, rect.top + half, rect.width - border, rect.height - border, Math.max(0, radius - half));
      ctx.stroke();
      ctx.restore();
    });
  }
}

/** One line of text, cut with an ellipsis where the box ends. */
function text(node: Text, cs: CSSStyleDeclaration, ops: ((ctx: CanvasRenderingContext2D) => void)[]): void {
  const range = document.createRange();
  range.selectNodeContents(node);
  const line = range.getClientRects()[0];
  const box = node.parentElement!.getBoundingClientRect();
  if (!line) return;
  const value = node.textContent!.replace(/\s+/g, " ").trim();
  const font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
  const color = cs.color;
  ops.push((ctx) => {
    ctx.font = font;
    ctx.fillStyle = color;
    ctx.textBaseline = "middle";
    const room = box.right - line.left;
    let shown = value;
    if (ctx.measureText(shown).width > room) {
      while (shown && ctx.measureText(`${shown}…`).width > room) shown = shown.slice(0, -1);
      shown = `${shown}…`;
    }
    ctx.fillText(shown, line.left, line.top + line.height / 2);
  });
}

function drawImage(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  rect: DOMRect,
  fit: string,
  radius: number,
  faded: boolean,
): void {
  const iw = img.naturalWidth;
  const ih = img.naturalHeight;
  const k = fit === "cover" ? Math.max(rect.width / iw, rect.height / ih) : fit === "contain" ? Math.min(rect.width / iw, rect.height / ih) : 0;
  const dw = k ? iw * k : rect.width;
  const dh = k ? ih * k : rect.height;
  const dx = rect.left + (rect.width - dw) / 2;
  const dy = rect.top + (rect.height - dh) / 2;
  ctx.save();
  ctx.beginPath();
  ctx.roundRect(rect.left, rect.top, rect.width, rect.height, radius);
  ctx.clip();
  if (faded) {
    // The card backdrop icon fades out towards the bottom-right (its CSS mask).
    const layer = document.createElement("canvas");
    layer.width = Math.ceil(rect.width * 2);
    layer.height = Math.ceil(rect.height * 2);
    const l = layer.getContext("2d")!;
    l.scale(2, 2);
    l.drawImage(img, dx - rect.left, dy - rect.top, dw, dh);
    l.globalCompositeOperation = "destination-in";
    const g = l.createLinearGradient(0, 0, rect.width, rect.height);
    g.addColorStop(0.3, "#000");
    g.addColorStop(0.85, "transparent");
    l.fillStyle = g;
    l.fillRect(0, 0, rect.width, rect.height);
    ctx.drawImage(layer, rect.left, rect.top, rect.width, rect.height);
  } else {
    ctx.drawImage(img, dx, dy, dw, dh);
  }
  ctx.restore();
}

/** The SVG with its stylesheet colours written onto each shape, so it renders on its own. */
function inlineSvg(svg: SVGSVGElement): string {
  const copy = svg.cloneNode(true) as SVGSVGElement;
  const from = [svg, ...svg.querySelectorAll("*")];
  const to = [copy, ...copy.querySelectorAll("*")];
  from.forEach((el, i) => {
    const cs = getComputedStyle(el);
    for (const prop of ["fill", "stroke", "stroke-width", "font-family", "font-size", "font-weight"]) {
      (to[i] as SVGElement).style.setProperty(prop, cs.getPropertyValue(prop));
    }
  });
  const box = svg.getBoundingClientRect();
  copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  copy.setAttribute("width", String(box.width));
  copy.setAttribute("height", String(box.height));
  return new XMLSerializer().serializeToString(copy);
}

function isPainted(color: string): boolean {
  return color !== "transparent" && !/rgba\(.*,\s*0\)$/.test(color);
}
