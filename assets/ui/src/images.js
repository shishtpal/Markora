import { $, showError } from "./dom.js";

const CHUNK = 256 * 1024;
const imageName = (name) => /\.(png|jpe?g|gif|webp|bmp|svg|ico|avif|tiff?)$/i.test(name);
const isImage = (file) => file.type.startsWith("image/") || imageName(file.name);

export function createImages(bridge, editor, getSnapshot) {
  let nextToken = 0,
    pending = null,
    lastAck = -1,
    finishing = false,
    retry = null;
  const picker = $("#image-picker");
  const notice = $("#image-import-note");
  const retryButton = $("#image-import-retry");
  const send = (request, draft = false) => bridge.send({ type: "image", request }, draft);
  const tabPath = (id) => getSnapshot()?.tabs.find((tab) => tab.id === id)?.path;
  function message(text, canRetry = false) {
    $("#image-import-text").textContent = text;
    retryButton.hidden = !canRetry;
    notice.hidden = false;
  }
  function begin(files, paths = null, target = null) {
    if (pending) {
      showError("Wait for the current image import to finish.");
      return;
    }
    const anchor = editor.captureImageAnchor(target);
    if (!anchor) {
      showError("Images can be inserted into editable Markdown notes.");
      return;
    }
    if (
      !files.length ||
      files.length > 10 ||
      (!paths &&
        (files.some((f) => !f.size || f.size > 20 * 1024 * 1024) ||
          files.reduce((sum, f) => sum + f.size, 0) > 50 * 1024 * 1024))
    ) {
      showError("Choose up to 10 images, at most 20 MiB each and 50 MiB together.");
      return;
    }
    retry = null;
    lastAck = -1;
    finishing = false;
    pending = { token: ++nextToken, anchor, files, offset: 0 };
    message(
      tabPath(anchor.docId)
        ? "Importing images…"
        : "Save this note to choose where its images will be stored.",
    );
    send(
      {
        op: paths ? "paths" : "begin",
        token: pending.token,
        docId: anchor.docId,
        revision: anchor.revision,
        ...(paths ? { paths } : { files: files.map((f) => ({ name: f.name, size: f.size })) }),
      },
      true,
    );
  }
  async function upload(reply) {
    const operation = pending;
    try {
      const file = operation.files[reply.fileIndex];
      const bytes = new Uint8Array(
        await file.slice(operation.offset, operation.offset + CHUNK).arrayBuffer(),
      );
      if (operation !== pending) return;
      let binary = "";
      for (let i = 0; i < bytes.length; i += 8192)
        binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
      operation.offset += bytes.length;
      const last = operation.offset === file.size;
      if (last) operation.offset = 0;
      send({
        op: "chunk",
        token: operation.token,
        seq: reply.nextSeq,
        fileIndex: reply.fileIndex,
        data: btoa(binary),
        last,
      });
    } catch (error) {
      showError(`Could not read the image: ${error.message}`);
      send({ op: "dismiss", token: operation.token });
      pending = null;
      notice.hidden = true;
    }
  }
  function accept(snapshot) {
    const reply = snapshot.imageImport;
    if (!pending || !reply || reply.token !== pending.token) return;
    if (reply.status === "ready" && lastAck !== reply.nextSeq) {
      lastAck = reply.nextSeq;
      if (reply.fileIndex < pending.files.length) upload(reply);
      else if (!finishing) {
        finishing = true;
        send({ op: "finish", token: pending.token });
      }
    } else if (reply.status === "complete" || reply.status === "stale") {
      const operation = pending;
      pending = null;
      if (
        tabPath(operation.anchor.docId) === reply.documentPath &&
        editor.insertImages(operation.anchor, reply.images)
      )
        notice.hidden = true;
      else {
        retry = { docId: reply.docId, path: reply.documentPath, images: reply.images };
        message("Images saved. Return to the original note and insert them at your cursor.", true);
      }
      send({ op: "dismiss", token: reply.token });
    } else if (reply.status === "error") {
      pending = null;
      notice.hidden = true;
      showError(reply.message);
      send({ op: "dismiss", token: reply.token });
    }
  }
  retryButton.onclick = () => {
    const anchor = editor.captureImageAnchor();
    if (!retry || !anchor || anchor.docId !== retry.docId || tabPath(anchor.docId) !== retry.path) {
      showError("Return to the original note before inserting its imported images.");
      return;
    }
    if (editor.insertImages(anchor, retry.images)) {
      retry = null;
      notice.hidden = true;
    }
  };
  $("#image-import-dismiss").onclick = () => {
    if (!pending) {
      retry = null;
      notice.hidden = true;
    }
  };
  picker.onchange = () => {
    const files = [...picker.files];
    picker.value = "";
    if (files.length) begin(files);
  };
  document.addEventListener("paste", (event) => {
    if (!event.target.closest?.("#document")) return;
    const files = [...(event.clipboardData?.items || [])]
      .filter((i) => i.kind === "file")
      .map((i) => i.getAsFile())
      .filter(Boolean);
    if (!files.some(isImage)) return;
    event.preventDefault();
    event.stopPropagation();
    begin(files, null, event.target);
  });
  // DOM drops cover image blobs from other apps; native file paths arrive through Wry once.
  document.addEventListener("dragover", (event) => event.preventDefault());
  document.addEventListener("drop", (event) => {
    event.preventDefault();
    const files = [...(event.dataTransfer?.files || [])];
    if (files.some(isImage)) {
      if (!event.target.closest?.("#document")) {
        showError("Drop images into the document workspace.");
        return;
      }
      begin(files, null, event.target);
    } else {
      for (const file of files) {
        const path = file.path || window.chrome?.webview?.getFilePath?.(file);
        if (path) bridge.send({ type: "openPath", path });
      }
    }
  });
  function nativeDrop(paths, position) {
    const images = paths.filter(imageName);
    if (images.length && images.length !== paths.length) {
      showError("Drop images and documents separately.");
      return;
    }
    if (images.length) {
      const target = document.elementFromPoint(
        position[0] / devicePixelRatio,
        position[1] / devicePixelRatio,
      );
      if (!target?.closest("#document")) {
        showError("Drop images into the document workspace.");
        return;
      }
      begin(
        images.map((name) => ({ name })),
        images,
        target,
      );
    } else for (const path of paths) bridge.send({ type: "openPath", path });
  }
  return { accept, nativeDrop, pick: () => picker.click() };
}
