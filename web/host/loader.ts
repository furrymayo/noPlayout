import type { GraphicsManifest } from 'ograf';
import type { LoadParams } from './protocol';
import type { CreatedGraphic, Graphic } from './runtime';

interface LoaderAdapters {
  document: Pick<Document, 'createElement'>;
  registry: Pick<CustomElementRegistry, 'define' | 'get'>;
  stage: HTMLElement;
  baseUrl: string;
  fetchManifest(url: string): Promise<unknown>;
  importModule(url: string): Promise<{ default: CustomElementConstructor }>;
}
const registrations = new WeakMap<object, WeakMap<CustomElementConstructor, string>>();
let nextTag = 0;
const absolute = (element: HTMLElement) => Object.assign(element.style, { position: 'absolute', inset: '0', width: '100%', height: '100%' });

/** Loads packaged ES modules natively: neither main nor its relative imports are bundled. */
export class GraphicLoader {
  private readonly layers = new Map<number, HTMLElement>();
  constructor(private readonly adapters: LoaderAdapters) {}
  async create(params: LoadParams): Promise<CreatedGraphic> {
    const a = this.adapters;
    const url = new URL(params.graphicUrl, a.baseUrl);
    const raw = await a.fetchManifest(url.href);
    if (!raw || typeof raw !== 'object') throw new Error('Invalid OGraf manifest');
    const manifest = raw as GraphicsManifest;
    if (typeof manifest.main !== 'string' || !manifest.main || manifest.supportsRealTime !== true)
      throw new Error('OGraf manifest must declare a main module and realtime support');
    const module = await a.importModule(new URL(manifest.main, url).href);
    if (typeof module.default !== 'function') throw new Error('OGraf main must default-export a custom element class');
    let constructors = registrations.get(a.registry);
    if (!constructors) { constructors = new WeakMap(); registrations.set(a.registry, constructors); }
    let tag = constructors.get(module.default);
    if (!tag) {
      do { tag = `noplayout-graphic-${++nextTag}`; } while (a.registry.get(tag));
      a.registry.define(tag, module.default); constructors.set(module.default, tag);
    }
    const graphic = a.document.createElement(tag);
    for (const method of ['load', 'dispose', 'playAction', 'stopAction', 'updateAction', 'customAction'])
      if (typeof (graphic as unknown as Record<string, unknown>)[method] !== 'function') throw new Error('Missing OGraf method');
    absolute(graphic); graphic.style.display = 'block';
    const slot = a.document.createElement('div'); absolute(slot);
    slot.style.isolation = 'isolate'; slot.dataset.layer = String(params.layer);
    const resolution = params.renderCharacteristics.resolution;
    if (resolution) { slot.style.width = `${resolution.width}px`; slot.style.height = `${resolution.height}px`; }
    slot.appendChild(graphic);
    if (params.cued) {
      // visibility:hidden retains layout/ResizeObserver/font sizing; display:none does not.
      slot.style.visibility = 'hidden'; a.stage.appendChild(slot);
    } else {
      let layer = this.layers.get(params.layer);
      if (!layer) {
        layer = a.document.createElement('div'); absolute(layer);
        layer.style.zIndex = String(params.layer); layer.dataset.layer = String(params.layer);
        a.stage.appendChild(layer); this.layers.set(params.layer, layer);
      }
      layer.appendChild(slot);
    }
    return {
      graphic: graphic as unknown as Graphic,
      manifest: manifest as CreatedGraphic['manifest'],
      surface: {
        show(layer) { slot.style.zIndex = String(layer); slot.dataset.layer = String(layer); slot.style.visibility = 'visible'; },
        remove() { slot.remove(); },
      },
    };
  }
}

export function browserLoader(stage: HTMLElement): GraphicLoader {
  return new GraphicLoader({ document, registry: customElements, stage, baseUrl: document.baseURI,
    async fetchManifest(url) {
      const response = await fetch(url);
      if (!response.ok) throw new Error('Graphic manifest request failed');
      return response.json();
    },
    importModule: (url) => import(/* @vite-ignore */ url),
  });
}
