// A8 — the keyboard activates what the click activates: Enter and
// Space, on any element that carries role="button" without being a
// <button> (the rows and chips whose prototype geometry forbids the
// native element). The focus ring lives in system.css (:focus-visible).
export const activation = (run) => (event) => {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    run();
  }
};
