package android.doremi

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * JSON serialization/deserialization
 */
object JsonSerde {
    fun noteFromJson(elem: JsonElement): Note {
        val obj = elem.jsonObject
        return Note(
            id = obj["id"]?.jsonPrimitive?.content?.toULongOrNull() ?: 0u,
            name = obj["name"]?.jsonPrimitive?.content ?: "",
            tags = obj["tags"]?.jsonArray?.map { it.jsonPrimitive.content }
                ?.filter { it.isNotBlank() }?.takeIf { it.isNotEmpty() } ?: emptyList(),
            body = obj["body"]?.jsonPrimitive?.content ?: "",
            ctime = obj["ctime"]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L)
    }

    fun parseNotes(json: String): List<Note> {
        return try {
            val jsonElement = Json.parseToJsonElement(json)
            val jsonArray = jsonElement.jsonArray
            jsonArray.map { element -> noteFromJson(element) }
        } catch (e: Exception) {
            System.err.println("Failed to parse notes JSON: ${e.message}")
            emptyList()
        }
    }

    fun parseNote(json: String): Note? {
        return try {
            if (json.isEmpty()) return null
            noteFromJson(Json.parseToJsonElement(json))
        } catch (e: Exception) {
            System.err.println("Failed to parse note JSON: ${e.message}")
            null
        }
    }
}
