import { z } from "zod";
import { AutoRouter, json } from "itty-router";

/// schema validation
const userSchema = z.object({
  name: z
    .string()
    .min(2, "Name must be at least 2 characters.")
    .regex(/^[a-zA-Z ]+$/, "Name can only contain letters and spaces."),
  age: z.number().min(18),
});

/// define a RESTful API
const router = AutoRouter();

router.get("/user/:name", async (request) => {
  return json({
    message: `GET request for '${request.params.name}' successful 🚀`,
  });
});

router.post("/user", async (request) => {
  const body = await request.json();
  const parsed = userSchema.safeParse(body);
  if (!parsed.success) {
    return new Response(parsed.error.message, { status: 418 });
  }
  return json({ message: "POST request successful 🚀", data: parsed.data });
});

/// handler
export default router.fetch;
