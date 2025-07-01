import { AutoRouter, json } from "itty-router";
import { z } from "zod";

/// schema validation
const userSchema = z.object({
  name: z
    .string()
    .min(2, "Name must be at least 2 characters.")
    .regex(/^[a-zA-Z ]+$/, "Name can only contain letters and spaces."),
  age: z.number().min(0),
  email: z.string().email().optional(),
});

/// define a RESTful API
const router = AutoRouter();

router.get("/user/:name", async (request) => {
  let name = decodeURIComponent(request.params.name);
  // Keys cannot contain space characters
  const value = Kv.get(name.replace(" ", "_"));
  if (!value) {
    return new Response("User not found", { status: 404 });
  }
  const user = userSchema.parse(value);
  return json(user);
});

router.post("/user", async (request) => {
  const body = await request.json();
  const parsed = userSchema.safeParse(body);
  if (!parsed.success) {
    return new Response(parsed.error.message, { status: 422 });
  }
  const user = parsed.data;
  let name = decodeURIComponent(user.name);
  // Keys cannot contain space characters
  Kv.set(name.replace(" ", "_"), user);
  return json({ message: `user '${user.name}' successfully created 🚀` });
});

/// entrypoint handler
const handler = (request: Request): Promise<Response> => router.fetch(request);

export default handler;
