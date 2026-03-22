package android.doremi

/**
 * JNI wrapper for the Rust doremi core library.
 */
object CoreLib {
    init {
        try {
            System.loadLibrary("doremi")
        } catch (e: UnsatisfiedLinkError) {
            System.err.println("Failed to load native library: ${e.message}")
        }
    }

    /**
     * @brief Read all notes
     * 
     * See Java_android_doremi_CoreLib_readAll
     *
     * @param[in] basedir App's files directory
     * 
     * @return JSON List<Note>
     */
    external fun readAll(basedir: String): String

    /**
     * @brief Create new note
     * 
     * See Java_android_doremi_CoreLib_new
     *
     * @param[in] basedir App's files directory
     * @param[in] name Note's name
     * @param[in] tags Array of tag strings
     * @param[in] body Note content
     * @param[in] dbgCTime Debug creation time
     * 
     * @return JSON Note
     */
    external fun new(basedir: String, name: String, tags: Array<String>, body: String, dbgCTime: Long): String

    /**
     * @brief Edit an existing note
     * 
     * See Java_android_doremi_CoreLib_update
     * 
     * @param[in] basedir App's files directory
     * @param[in] id Note's ID
     * @param[in] name Note's new name
     * @param[in] tags Array of new tag strings
     * @param[in] body New note content
     * 
     * @return JSON Note
     */
    external fun update(basedir: String, id: String, name: String, tags: Array<String>, body: String): String
}
