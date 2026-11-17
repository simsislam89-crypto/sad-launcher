import { Composition } from "remotion";
import { Speed } from "./cards/Speed";
import { Effects } from "./cards/Effects";
import { Friend } from "./cards/Friend";
import { Pipeline } from "./cards/Pipeline";
import { Search } from "./cards/Search";
import { Loaders } from "./cards/Loaders";

// Looping card clips for the website's «Что внутри» grid (site/assets/cards/).
// Black background on purpose: the page blends them with `screen`.
export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition id="Speed" component={Speed} durationInFrames={150} fps={30} width={960} height={540} />
      <Composition id="Effects" component={Effects} durationInFrames={150} fps={30} width={960} height={540} />
      <Composition id="Friend" component={Friend} durationInFrames={180} fps={30} width={960} height={540} />
      <Composition id="Pipeline" component={Pipeline} durationInFrames={150} fps={30} width={960} height={540} />
      <Composition id="Search" component={Search} durationInFrames={150} fps={30} width={960} height={540} />
      <Composition id="Loaders" component={Loaders} durationInFrames={180} fps={30} width={960} height={540} />
    </>
  );
};
