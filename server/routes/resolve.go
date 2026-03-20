package routes

import "github.com/prvious/kontainer/server/app/facades"

// resolve is a generic helper that resolves a singleton from the service
// container and type-asserts it. Returns the zero value and false on failure.
func resolve[T any](key string) (T, bool) {
	instance, err := facades.App().Make(key)
	if err != nil {
		facades.Log().Warningf("failed to resolve %s: %v", key, err)
		var zero T
		return zero, false
	}
	val, ok := instance.(T)
	if !ok {
		facades.Log().Warningf("%s has unexpected type %T", key, instance)
		var zero T
		return zero, false
	}
	return val, true
}
