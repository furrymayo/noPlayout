// Pixel fixture (M1 task 10): shapes and gradients only, no text, so one golden serves every OS
// within ±2 LSB. A 1024 × 256 rectangle with a horizontal red → green → blue gradient under a
// vertical alpha gradient (1 → 0), an antialiased circle at half alpha, and a square rotated
// 30° with a 2 px stroke. Static: painted once on load.
class GradientShape extends HTMLElement {
  async load(params) {
    if (params.renderType !== "realtime") {
      return { statusCode: 400, statusMessage: "realtime only" };
    }
    const c = document.createElement("canvas");
    c.width = window.innerWidth;
    c.height = window.innerHeight;
    c.style.cssText = "position:absolute;left:0;top:0;width:100%;height:100%";
    this.appendChild(c);
    const ctx = c.getContext("2d", { alpha: true });

    // Colour gradient, then an alpha gradient applied with destination-in.
    const g = ctx.createLinearGradient(128, 0, 1152, 0);
    g.addColorStop(0, "#ff0000");
    g.addColorStop(0.5, "#00ff00");
    g.addColorStop(1, "#0000ff");
    ctx.fillStyle = g;
    ctx.fillRect(128, 128, 1024, 256);
    const a = ctx.createLinearGradient(0, 128, 0, 384);
    a.addColorStop(0, "rgba(0,0,0,1)");
    a.addColorStop(1, "rgba(0,0,0,0)");
    ctx.globalCompositeOperation = "destination-in";
    ctx.fillStyle = a;
    ctx.fillRect(128, 128, 1024, 256);
    ctx.globalCompositeOperation = "source-over";

    ctx.fillStyle = "rgba(255, 200, 0, 0.5)";
    ctx.beginPath();
    ctx.arc(480, 720, 160, 0, 2 * Math.PI);
    ctx.fill();

    ctx.save();
    ctx.translate(1200, 720);
    ctx.rotate(Math.PI / 6);
    ctx.fillStyle = "#3070e0";
    ctx.fillRect(-120, -120, 240, 240);
    ctx.lineWidth = 2;
    ctx.strokeStyle = "#ffffff";
    ctx.strokeRect(-120, -120, 240, 240);
    ctx.restore();
    return { statusCode: 200 };
  }

  async playAction() {
    return { statusCode: 200, currentStep: 0 };
  }

  async updateAction() {
    return { statusCode: 200 };
  }

  async stopAction() {
    return { statusCode: 200 };
  }

  async customAction() {
    return { statusCode: 400, statusMessage: "no custom actions" };
  }

  async dispose() {
    this.replaceChildren();
    return { statusCode: 200 };
  }
}

export default GradientShape;
