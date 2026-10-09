// The work goes back where the repository came from: the branch pushed to origin (GitHub…), then
// a link to open its pull request.
import { Button } from "../../../components/Button";
import { Icon } from "../../../components/Icon";
import type { MachineActions } from "../actions";

export function PushBranch({ actions: a }: { actions: MachineActions }) {
  const pushed = a.push.data;
  return (
    <div className="push-branch">
      <p>{pushed ? `On ${pushed.remote}` : "Or send the branch to the repository's origin, for a pull request:"}</p>
      {pushed?.pull_request ? (
        <a className="btn btn-primary btn-md" href={pushed.pull_request} target="_blank" rel="noopener">
          <Icon name="external" />
          Open pull request
        </a>
      ) : (
        <Button disabled={a.push.isPending || !!pushed} onClick={() => a.push.mutate()}>
          <Icon name="cloud-up" />
          {a.push.isPending ? "Pushing…" : pushed ? "Pushed" : "Push branch"}
        </Button>
      )}
    </div>
  );
}
