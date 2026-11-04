import { IrisError, Spec } from "gecode-iris";

const spec = Spec.parse("n:int row[n]:int", "example.iris");

export default {
  async fetch(request: Request): Promise<Response> {
    try {
      const data = spec.read(new Uint8Array(await request.arrayBuffer()), "request body");
      return new Response(data.toJSON(), { headers: { "Content-Type": "application/json" } });
    } catch (error) {
      if (!(error instanceof IrisError)) throw error;
      return new Response(error.toJSON(), {
        status: 422,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
