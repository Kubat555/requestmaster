<script lang="ts" module>
  /** What the menu of a row can do with its node. */
  export type TreeAction = "new_request" | "new_folder" | "rename" | "duplicate" | "delete";
</script>

<script lang="ts">
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import { strings } from "../lib/strings";
  import type { NodeKind } from "../lib/types";

  interface Props {
    /** Kind of the node the menu is for: a request has nothing to create in. */
    kind: NodeKind;
    /** Where the menu appears: under this element, or at this point of the window (a right click). */
    anchor: HTMLElement | { x: number; y: number };
    /** The user chose an action; the menu is already closing. */
    onaction: (action: TreeAction) => void;
    /** Called when the menu must go away; the owner removes it. */
    onclose: () => void;
  }

  let { kind, anchor, onaction, onclose }: Props = $props();

  const items = $derived.by<MenuItem[]>(() => {
    const item = (action: TreeAction, label: string, more: Partial<MenuItem> = {}): MenuItem => ({
      label,
      ...more,
      onselect: () => onaction(action),
    });
    const create =
      kind === "request"
        ? []
        : [
            item("new_request", strings.tree.newNode.request, { icon: "plus" }),
            item("new_folder", strings.tree.newNode.folder, { icon: "folder" }),
          ];
    return [
      ...create,
      item("rename", strings.tree.rename, { icon: "edit" }),
      item("duplicate", strings.tree.duplicate, { icon: "duplicate" }),
      item("delete", strings.tree.delete, { icon: "trash", danger: true }),
    ];
  });
</script>

<!-- The menu of a row of the tree. Drawn outside the row, and no key pressed in it reaches anything around:
     Delete or F2 in the menu are not commands of the tree. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="tree-menu" onkeydown={(event) => event.stopPropagation()}>
  <Menu label={strings.tree.menu} {items} {anchor} {onclose} />
</div>

<style>
  .tree-menu {
    display: contents;
  }
</style>
