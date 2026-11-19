// Co-located Web native HTML <meter> + <input type="number"> + <canvas> dial.

export function mount({ props, actions, theme, effect, onCleanup }) {
  const card = document.createElement("section");
  card.style.cssText =
    "display:flex;flex-direction:column;gap:12px;padding:14px;border-radius:10px;border:1px solid var(--bern-control-border, #ccc);";

  const header = document.createElement("div");
  header.style.cssText = "display:flex;justify-content:space-between;align-items:center;";
  const heading = document.createElement("strong");
  const readout = document.createElement("span");
  readout.style.fontVariantNumeric = "tabular-nums";
  header.append(heading, readout);

  const row = document.createElement("div");
  row.style.cssText = "display:flex;align-items:center;gap:12px;";

  const meter = document.createElement("meter");
  meter.min = String(props.levelMin);
  meter.max = String(props.levelMax);
  meter.low = "3";
  meter.high = "7";
  meter.optimum = "5";
  meter.style.cssText = "flex:1;height:20px;cursor:pointer;";

  const onMove = (e) => {
    const rect = meter.getBoundingClientRect();
    if (rect.width > 1) {
      const frac = Math.min(Math.max((e.clientX - rect.left) / rect.width, 0), 1);
      actions.setHover(Math.round(frac * props.levelMax * 10) / 10);
    }
  };
  const onLeave = () => actions.setHover(null);
  const onClick = (e) => {
    const rect = meter.getBoundingClientRect();
    if (rect.width > 1) {
      const frac = Math.min(Math.max((e.clientX - rect.left) / rect.width, 0), 1);
      actions.setLevel(Math.round(frac * props.levelMax));
    }
  };
  meter.addEventListener("pointermove", onMove);
  meter.addEventListener("pointerleave", onLeave);
  meter.addEventListener("click", onClick);

  const number = document.createElement("input");
  number.type = "number";
  number.min = String(props.levelMin);
  number.max = String(props.levelMax);
  number.step = "1";
  number.style.width = "72px";
  const onNumber = () => {
    const val = Number(number.value);
    if (Number.isFinite(val)) actions.setLevel(val);
  };
  number.addEventListener("change", onNumber);
  row.append(meter, number);

  const footer = document.createElement("div");
  footer.style.cssText = "display:flex;justify-content:space-between;align-items:center;gap:12px;";
  const label = document.createElement("label");
  label.style.cssText = "display:inline-flex;align-items:center;gap:6px;";
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  const checkboxText = document.createElement("span");
  const onCheck = () => actions.setBoosted(checkbox.checked);
  checkbox.addEventListener("change", onCheck);
  label.append(checkbox, checkboxText);

  const resetBtn = document.createElement("button");
  resetBtn.type = "button";
  const onReset = () => actions.reset();
  resetBtn.addEventListener("click", onReset);
  footer.append(label, resetBtn);

  card.append(header, row, footer);

  effect(() => {
    heading.textContent = props.heading();
    readout.textContent = props.readout();
    readout.style.color = theme.textSecondary();
    card.style.borderColor = theme.border();
    const val = props.level();
    meter.value = val;
    if (Number(number.value) !== val) number.value = String(val);
    checkbox.checked = props.boosted();
    checkboxText.textContent = props.boostedLabel();
    resetBtn.textContent = props.resetLabel();
  });

  onCleanup(() => {
    meter.removeEventListener("pointermove", onMove);
    meter.removeEventListener("pointerleave", onLeave);
    meter.removeEventListener("click", onClick);
    number.removeEventListener("change", onNumber);
    checkbox.removeEventListener("change", onCheck);
    resetBtn.removeEventListener("click", onReset);
  });

  return card;
}
