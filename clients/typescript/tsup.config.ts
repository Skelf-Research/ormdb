import { defineConfig } from "tsup";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    "prisma/index": "src/prisma/index.ts",
    "drizzle/index": "src/drizzle/index.ts",
    "typeorm/index": "src/typeorm/index.ts",
    "kysely/index": "src/kysely/index.ts",
    "sequelize/index": "src/sequelize/index.ts",
  },
  format: ["cjs", "esm"],
  dts: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  treeshake: true,
  minify: false,
});
