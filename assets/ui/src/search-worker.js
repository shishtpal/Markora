import { searchDocument } from "./search-engine.js";

// Regex execution never runs on the UI thread, including patterns with backtracking.
export function createSearchRunner() {
  const source = `self.onmessage=({data})=>{try{self.postMessage({result:(${searchDocument.toString()})(data)});}catch(error){self.postMessage({error:error.message});}};`;
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  let cancel = () => {};
  return {
    run(request) {
      cancel();
      return new Promise((resolve) => {
        let worker, timeout;
        const finish = (result) => {
          clearTimeout(timeout);
          worker?.terminate();
          cancel = () => {};
          resolve(result);
        };
        cancel = () => finish(null);
        try {
          worker = new Worker(url);
          worker.onmessage = ({ data }) => finish(data.error ? { error: data.error } : data.result);
          worker.onerror = (event) => {
            event.preventDefault();
            finish({ error: event.message || "Search could not start. Try again." });
          };
          timeout = setTimeout(
            () => finish({ error: "Search took too long. Simplify the expression and try again." }),
            1500,
          );
          worker.postMessage(request);
        } catch (error) {
          finish({ error: error.message });
        }
      });
    },
    cancel() {
      cancel();
    },
    destroy() {
      cancel();
      URL.revokeObjectURL(url);
    },
  };
}
