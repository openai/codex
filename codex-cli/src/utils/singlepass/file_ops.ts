import { z } from "zod";

/**
 * Represents a file operation, including modifications, deletes, and moves.
 */
export const FileOperationSchema = z.object({
  /**
   * Absolute path to the file.
   */
  path: z.string(),

  /**
   * FULL CONTENT of the file after modification. Provides the FULL AND FINAL content of
   * the file after modification WITHOUT OMITTING OR TRUNCATING ANY PART OF THE FILE.
   *
   * Mutually exclusive with 'delete' and 'move_to'.
   */
  updated_full_content: z.string().nullable().optional(),

  /**
   * Set to true if the file is to be deleted.
   *
   * Mutually exclusive with 'updated_full_content' and 'move_to'.
   */
  delete: z.boolean().nullable().optional(),

  /**
   * New path of the file if it is to be moved.
   *
   * Mutually exclusive with 'updated_full_content' and 'delete'.
   */
  move_to: z.string().nullable().optional(),
});

export type FileOperation = z.infer<typeof FileOperationSchema>;

/**
 * Container for one or more FileOperation objects.
 */
export const EditedFilesSchema = z.object({
  /**
   * A list of file operations that are applied in order.
   */
  ops: z.array(FileOperationSchema),
});

export type EditedFiles = z.infer<typeof EditedFilesSchema>;
