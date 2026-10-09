// Opens the New VM launcher from anywhere: the sidebar, the wall, ⌘K.
export const NEW_VM_EVENT = "agentvm:new-vm";
export const openLauncher = () => window.dispatchEvent(new Event(NEW_VM_EVENT));
