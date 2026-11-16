// A synthetic big collection for the tests of the tree. Not imported by the application.

import type { NodeDto } from "../types";

/**
 * One collection with `folders` folders of `requestsPerFolder` requests each. The nodes go in the order of the
 * fully expanded tree: the collection, then every folder followed by its requests. Every name is unique and is
 * not a part of another name, so a filter by the name of a request finds that request alone.
 */
export function bigTree(folders: number, requestsPerFolder: number): NodeDto[] {
  const number = (value: number, count: number) => String(value + 1).padStart(String(count).length, "0");
  const nodes: NodeDto[] = [{ id: "big-col", parent_id: null, kind: "collection", name: "Big collection", content: {} }];
  for (let folder = 0; folder < folders; folder++) {
    const folderNumber = number(folder, folders);
    const folderId = `big-fold-${folderNumber}`;
    nodes.push({ id: folderId, parent_id: "big-col", kind: "folder", name: `Folder ${folderNumber}`, content: {} });
    for (let request = 0; request < requestsPerFolder; request++) {
      const requestNumber = `${folderNumber}-${number(request, requestsPerFolder)}`;
      nodes.push({
        id: `big-req-${requestNumber}`,
        parent_id: folderId,
        kind: "request",
        name: `Request ${requestNumber}`,
        content: { method: "GET", url: `https://api.example.test/${requestNumber}` },
      });
    }
  }
  return nodes;
}
