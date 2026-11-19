// Co-located Web native HTML <input type="date"> picker.

export function mount({ props, actions, theme, effect, onCleanup }) {
  const card = document.createElement("section");
  card.style.cssText =
    "display:flex;flex-direction:column;gap:12px;padding:14px;border-radius:10px;border:1px solid var(--bern-control-border, #ccc);";

  const header = document.createElement("div");
  header.style.cssText = "display:flex;justify-content:space-between;align-items:center;gap:10px;";
  const heading = document.createElement("strong");
  const right = document.createElement("div");
  right.style.cssText = "display:flex;align-items:center;gap:10px;";
  const readout = document.createElement("span");
  readout.style.fontVariantNumeric = "tabular-nums";
  const todayBtn = document.createElement("button");
  todayBtn.type = "button";
  const onToday = () => actions.today();
  todayBtn.addEventListener("click", onToday);
  right.append(readout, todayBtn);
  header.append(heading, right);

  const input = document.createElement("input");
  input.type = "date";
  input.style.cssText = "padding:6px 10px;font-size:14px;max-width:220px;";
  const onChange = () => {
    if (input.value) actions.setDate(input.value);
  };
  input.addEventListener("change", onChange);

  card.append(header, input);

  effect(() => {
    heading.textContent = props.heading();
    const iso = props.date();
    readout.textContent = iso;
    readout.style.color = theme.textSecondary();
    card.style.borderColor = theme.border();
    todayBtn.textContent = props.todayLabel();
    if (input.value !== iso) input.value = iso;
  });

  onCleanup(() => {
    input.removeEventListener("change", onChange);
    todayBtn.removeEventListener("click", onToday);
  });

  return card;
}
