import { h } from "../../dom.js";

/** One figure in a row of facts: a small label over its value. */
export const fact = (label, value) => h("div", { class: "fact" }, h("span", {}, label), h("b", {}, value));
